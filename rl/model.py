"""Policy / value network for the STS2 combat environment.

Reads the flat observation of `sts2.VecEnv` through `sts2.layout()` and builds one token per entity: the player (with relics,
powers, orbs, Osty, global state), every enemy (with its intent, move history and the expert look-ahead of its upcoming turns),
every hand card, every potion, every candidate of a pending card selection and the three pile multisets. A small transformer mixes
the tokens; the action head is a *pointer* head that scores exactly the dense action space of the env (play card x target, use
potion x target, discard potion, pick, confirm, end turn), so a card is scored from its own token, never from a fixed input slot.
"""
import math
import os
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

import sts2

DEV = torch.device(os.environ.get("STS2_DEVICE", "cpu"))  # STS2_DEVICE=cuda runs the network on a GPU (observations stay numpy on the CPU side)
LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}


def S(x):
    """Signed log: unbounded game quantities (HP, damage, counters, powers ...) -> a small range, order-preserving."""
    return torch.sign(x) * torch.log1p(x.abs())


def sl(obs, name):
    o, s = SEC[name]
    return obs[:, o:o + s]


def mlp(i, h, o):
    return nn.Sequential(nn.Linear(i, h), nn.ReLU(), nn.Linear(h, o))


class CtxMLP(nn.Module):
    """mlp([x, ctx]) without materialising the concatenation: relu(x W_a + ctx W_b + b) W_2 (ctx is per sample, x per token)."""

    def __init__(self, d, h, o):
        super().__init__()
        self.a = nn.Linear(d, h)
        self.b = nn.Linear(d, h, bias=False)
        self.o = nn.Linear(h, o)

    def forward(self, x, ctx):  # x [B, N, d], ctx [B, d]
        return self.o(F.relu(self.a(x) + self.b(ctx).unsqueeze(1)))


class Bag(nn.Module):
    """Weighted sum of embeddings over a padded id list: sum_k emb_k(id_i) * w_k(i), done as one fused embedding_bag (no [B, L, e] temporaries)."""

    def __init__(self, n, e, channels):
        super().__init__()
        self.n, self.k = n + 1, channels
        self.emb = nn.Embedding(self.k * self.n, e, padding_idx=0)

    def forward(self, ids, w):
        """ids [..., L] long (0 = empty), w [..., L, k] float -> [..., e]"""
        lead = ids.shape[:-1]
        L = ids.shape[-1]
        ids = ids.reshape(-1, L)
        w = w.reshape(-1, L, self.k)
        offs = torch.arange(self.k, device=ids.device).view(1, 1, -1) * self.n
        full = (ids.unsqueeze(-1) + offs * (ids > 0).unsqueeze(-1)).reshape(-1, L * self.k)
        out = F.embedding_bag(full, self.emb.weight, per_sample_weights=w.reshape(-1, L * self.k) * (ids > 0).repeat_interleave(self.k, 1), mode="sum", padding_idx=0)
        return out.view(*lead, -1)


class PowerPool(nn.Module):
    """A creature's powers -> vector (embedding of the power scaled by features of its amount)."""

    def __init__(self, n_powers, e):
        super().__init__()
        self.bag = Bag(n_powers, e, 3)

    def forward(self, pw):  # pw [..., P, 2] (id+1, amount)
        pid = pw[..., 0].long().clamp(0, self.bag.n - 1)
        a = pw[..., 1]
        f = torch.stack([torch.ones_like(a), S(a) / 2.0, a.clamp(-10, 10) / 10.0], -1)
        return self.bag(pid, f)


class CardEnc(nn.Module):
    """Hand / candidate card -> d."""

    def __init__(self, d, e):
        super().__init__()
        self.card = nn.Embedding(C["N_CARDS"] + 1, e, padding_idx=0)
        self.upg = nn.Embedding(4, 8)
        self.ench = nn.Embedding(C["N_ENCHANTMENTS"] + 1, 8)
        self.aff = nn.Embedding(C["N_AFFLICTIONS"] + 1, 8)
        self.net = mlp(e + 24 + 8 + 13, 2 * d, d)

    def forward(self, f, extra):
        """f [..., 12] = id+1, upgrade, cost, playable, keywords, enchant, dmg, blk, c0, c1, ench_amt, afflic; extra [..., 3] = star cost, osty dmg, selected."""
        cid = f[..., 0].long().clamp(0, self.card.num_embeddings - 1)
        kw = f[..., 4].long().unsqueeze(-1)
        bits = ((kw >> torch.arange(8, device=f.device)) & 1).float()
        num = torch.stack([S(f[..., 2]), f[..., 3], S(f[..., 6]) / 2.0, S(f[..., 7]) / 2.0, S(f[..., 8]) / 2.0, S(f[..., 9]) / 2.0,
                           S(f[..., 10]) / 2.0, (f[..., 0] > 0).float(), S(extra[..., 0]), S(extra[..., 1]) / 2.0, (f[..., 2] < 0).float(),
                           torch.sign(f[..., 6]), extra[..., 2]], -1)
        x = torch.cat([self.card(cid), self.upg(f[..., 1].long().clamp(0, 3)), self.ench(f[..., 5].long().clamp(0, self.ench.num_embeddings - 1)),
                       self.aff(f[..., 11].long().clamp(0, self.aff.num_embeddings - 1)), bits, num], -1)
        return self.net(x)


class Net(nn.Module):
    """Entity encoders -> pooled context -> `rounds` of message passing -> pointer heads (see the module docstring)."""

    def __init__(self, d=64, e=24, rounds=2):
        super().__init__()
        self.d = d
        P = C["OBS_POWERS"]
        self.rounds = rounds
        self.pp = PowerPool(C["N_POWERS"], e)
        self.card = CardEnc(d, e)
        self.mon = nn.Embedding(C["N_MONSTERS"] + 1, e, padding_idx=0)
        self.kind = nn.Embedding(16, 8)
        self.node = nn.Embedding(C["LOOK_NODES"] + 8, 8, padding_idx=0)
        n_enemy_in = e + e + 7 + 3 * (8 + 3) + 4 * 8 + C["LOOK_H"] * (C["LOOK_NODES"] + 1)
        self.enemy = mlp(n_enemy_in, d, d)
        self.relic = Bag(C["N_RELICS"], e, 2)
        self.potion = nn.Embedding(C["N_POTIONS"] + 1, e, padding_idx=0)
        self.potion_enc = mlp(e + 1, d, d)
        self.orb = nn.Embedding(C["N_ORBS"] + 2, 4, padding_idx=0)
        self.pile = Bag(C["N_CARDS"], e, 2)  # (plain, upgraded) copies
        self.pile_enc = nn.ModuleList([mlp(e + 1, d, d) for _ in range(3)])
        n_player_in = 8 + 5 + 3 + 3 * e + C["MAX_ORBS"] * 4 + C["MAX_ORBS"] * 2 + 1 + 4 + e
        self.player = mlp(n_player_in, 2 * d, d)
        self.dec_src = nn.Embedding(10, 8)
        self.dec = mlp(8 + 7, d, d)
        # message passing: ctx = f(player, sums of enemies / hand / potions / cands, piles); token += g_type([token, ctx])
        self.ctx = nn.ModuleList([mlp(d * 9, d, d) for _ in range(rounds)])
        self.upd = nn.ModuleList([nn.ModuleDict({k: CtxMLP(d, d, d) for k in ("player", "enemy", "hand", "potion", "cand")}) for _ in range(rounds)])
        # heads
        self.u_card = mlp(d, d, d)
        self.v_tgt = mlp(d, d, d)
        self.v_none = nn.Parameter(torch.zeros(d))
        self.b_card = mlp(d, d, 1)
        self.u_pot = mlp(d, d, d)
        self.b_pot = mlp(d, d, 1)
        self.disc_pot = mlp(d, d, 1)
        self.pick = mlp(2 * d, d, 1)
        self.confirm = mlp(2 * d, d, 1)
        self.end = mlp(2 * d, d, 1)
        self.value = mlp(2 * d, 2 * d, 1)
        # the fight's HP-worth curve (rl/utility.py feats: U at 1/8 .. 8/8 of max HP), added to the player token; zero-initialised, so a network
        # trained before the input existed behaves exactly as before, and the input only matters once training has used it
        self.ucond = nn.Linear(8, d)
        nn.init.zeros_(self.ucond.weight)
        nn.init.zeros_(self.ucond.bias)
        self.register_buffer("lin_feats", torch.tensor([0.12, 0.25, 0.37, 0.50, 0.62, 0.75, 0.87, 1.00]), persistent=False)

    def encode(self, obs, E=None, L=None, has_dec=None):
        """`E` (enemy slots), `L` (pile entries) and `has_dec` (every row has a pending card selection / none has) fix the shapes: no device-to-host
        syncs, so the caller (which knows the batch from the host-side observation) can run it without stalls. None: derived from the batch."""
        B = obs.shape[0]
        H, E, K, Q = C["MAX_HAND"], C["OBS_MAX_ENEMIES"], C["MAX_POTIONS"], C["OBS_MAX_CANDS"]
        P = C["OBS_POWERS"]
        dev = obs.device
        g = sl(obs, "global")
        pl = sl(obs, "player")
        relics = sl(obs, "relics").view(B, -1, 2)
        potions = sl(obs, "potions").view(B, K, 2)
        hand = sl(obs, "hand").view(B, H, C["CARD_F"])
        enemies = sl(obs, "enemies").view(B, E, C["ENEMY_F"])
        dec = sl(obs, "decision")
        regent = sl(obs, "regent")
        osty = sl(obs, "osty")
        orbs = sl(obs, "orbs")
        look = sl(obs, "look").view(B, E, C["LOOK_H"], C["LOOK_NODES"] + 1)
        # only the enemy slots that are occupied somewhere in this batch (most fights have 1-3 enemies)
        if E is None:
            E = max(1, int((enemies[..., 0] > 0.5).any(0).nonzero().max().item() + 1)) if (enemies[..., 0] > 0.5).any() else 1
        enemies, look = enemies[:, :E], look[:, :E]
        # ---- player ----
        stage = g[:, 2:5]
        gsc = torch.stack([S(g[:, 0]) / 2.0, S(g[:, 1]) / 2.0, S(g[:, 6]) / 2.0, S(g[:, 7]) / 2.0, S(g[:, 8]) / 2.0], -1)
        sc = torch.stack([S(pl[:, 0]) / 3.0, pl[:, 0] / pl[:, 1].clamp(min=1), S(pl[:, 2]) / 3.0, S(pl[:, 3]), S(pl[:, 4]), S(pl[:, 5]),
                          S(pl[:, 6]), S(pl[:, 7])], -1)
        # all creatures' power lists in one pass: player, the 8 enemies, Osty
        pw = torch.cat([pl[:, 8:8 + 2 * P].view(B, 1, P, 2), enemies[..., 8:8 + 2 * P].reshape(B, E, P, 2), osty[:, 4:4 + 2 * P].view(B, 1, P, 2)], 1)
        pv = self.pp(pw)  # [B, 1 + E + 1, e]
        ppow, epow, opow = pv[:, 0], pv[:, 1:1 + E], pv[:, -1]
        rid = relics[..., 0].long().clamp(0, C["N_RELICS"])
        rb = self.relic(rid, torch.stack([torch.ones_like(relics[..., 1]), S(relics[..., 1]) / 2.0], -1))
        pid = potions[..., 0].long().clamp(0, C["N_POTIONS"])
        pb = self.potion(pid).sum(1)
        orb_kind = orbs[:, :C["MAX_ORBS"] * 3].view(B, C["MAX_ORBS"], 3)
        orb_e = self.orb(orb_kind[..., 0].long().clamp(0, C["N_ORBS"] + 1)).flatten(1)
        orb_v = torch.cat([S(orb_kind[..., 1:3].flatten(1)) / 2.0, S(orbs[:, -1:])], 1)
        osty_f = torch.cat([osty[:, :2], S(osty[:, 2:4]) / 3.0, opow], 1)
        player = self.player(torch.cat([sc, gsc, stage, ppow, rb, pb, orb_e, orb_v, osty_f], 1))
        # ---- enemies ----
        ep = enemies[..., 0] > 0.5
        mon = self.mon(enemies[..., 2].long().clamp(0, C["N_MONSTERS"]))
        esc = torch.stack([S(enemies[..., 3]) / 3.0, enemies[..., 3] / enemies[..., 4].clamp(min=1), S(enemies[..., 5]) / 3.0, S(enemies[..., 4]) / 3.0,
                           enemies[..., 6], enemies[..., 7], ep.float()], -1)
        iv = enemies[..., 8 + 2 * P:8 + 2 * P + 9].reshape(B, E, 3, 3)
        ie = self.kind(iv[..., 0].long().clamp(0, 15))
        inum = torch.stack([S(iv[..., 1]) / 2.0, S(iv[..., 2]), S(iv[..., 1] * iv[..., 2]) / 3.0], -1)
        perf = enemies[..., 8 + 2 * P + 9:8 + 2 * P + 13].long().clamp(0, C["LOOK_NODES"] + 7)
        pe = self.node(perf).flatten(2)
        lk = torch.cat([look[..., :-1], S(look[..., -1:]) / 3.0], -1).flatten(2)
        enemy = self.enemy(torch.cat([mon, epow, esc, torch.cat([ie, inum], -1).flatten(2), pe, lk], -1))
        cid = enemies[..., 1].long().clamp(0, C["MAX_CREATURES"] - 1)
        # ---- hand ----
        hp_ = hand[..., 0] > 0
        hand_t = self.card(hand, torch.stack([regent[:, :H], osty[:, -H:], torch.zeros_like(osty[:, -H:])], -1))
        # ---- potions ----
        pot_t = self.potion_enc(torch.cat([self.potion(pid), potions[..., 1:2]], -1))
        pot_p = pid > 0
        # ---- decision: candidates only for the envs that have a pending selection ----
        if has_dec is None:
            rows = (dec[:, 0] > 0.5).nonzero().squeeze(1)
        else:
            rows = torch.arange(B, device=dev) if has_dec else torch.zeros(0, dtype=torch.long, device=dev)
        cands = dec[rows, 8:].view(len(rows), Q, C["CARD_F"] + 1)
        rg = regent[rows, H:H + Q]
        cand_t = self.card(cands[..., :C["CARD_F"]], torch.stack([rg, torch.zeros_like(rg), cands[..., C["CARD_F"]]], -1))
        cand_p = cands[..., 0] > 0
        dh = torch.cat([self.dec_src(dec[:, 1].long().clamp(0, 9)), dec[:, 0:1], S(dec[:, 2:3]), S(dec[:, 3:4]), S(dec[:, 4:5]), dec[:, 5:6],
                        dec[:, 6:7], S(dec[:, 7:8])], 1)
        dec_t = self.dec(dh)
        # ---- piles: multiset of cards = bag of (plain / upgraded) card embeddings ----
        sizes = sl(obs, "pile_sizes")
        piles = []
        for k, nm in enumerate(["draw", "discard", "exhaust"]):
            pv_ = sl(obs, nm).view(B, -1, 2)
            Lk = L if L is not None else max(1, int((pv_[..., 0] > 0).sum(1).max().item()))
            pv_ = pv_[:, :Lk]
            ids = pv_[..., 0].long().clamp(0, C["N_CARDS"])
            up = (pv_[..., 1] > 0).float()
            w = torch.stack([1.0 - up, up], -1) / 4.0
            piles.append(self.pile_enc[k](torch.cat([self.pile(ids, w), S(sizes[:, k:k + 1]) / 3.0], 1)))
        return dict(player=player, enemy=enemy, hand=hand_t, potion=pot_t, cand=cand_t, piles=piles, dec=dec_t, ep=ep, hp=hp_, pot_p=pot_p,
                    cand_p=cand_p, cid=cid, rows=rows, cand_sel=cands[..., C["CARD_F"]] > 0.5)

    def trunk(self, obs, ufeat=None, **shape):
        """The pooled context the value head reads (`gctx` [B, 2d]): encoders + message passing. Heads trained on a frozen network
        (`rl/dist.py`, the end-HP distribution) use it."""
        return self.forward(obs, None, policy=False, value=False, _gctx=True, ufeat=ufeat, **shape)

    def forward(self, obs, mask, policy=True, value=True, _gctx=False, ufeat=None, **shape):
        """Returns (masked logits [B, ACTION_SPACE], value [B]); `policy=False` / `value=False` skips that head (None) and its cost.
        `shape`: E / L / has_dec of `encode`."""
        B = obs.shape[0]
        d = self.d
        E, Q = C["OBS_MAX_ENEMIES"], C["OBS_MAX_CANDS"]
        T = C["MAX_CREATURES"]
        z = self.encode(obs, **shape)
        player, enemy, hand, pot, cand = z["player"], z["enemy"], z["hand"], z["potion"], z["cand"]
        uf = self.lin_feats.to(player.dtype).expand(B, 8) if ufeat is None else ufeat.to(player.dtype)
        player = player + self.ucond(uf)
        ep, hp_, pot_p, cand_p = z["ep"].unsqueeze(-1), z["hp"].unsqueeze(-1), z["pot_p"].unsqueeze(-1), z["cand_p"].unsqueeze(-1)
        rows = z["rows"]
        E = enemy.shape[1]
        zeros_c = torch.zeros(B, d, device=obs.device, dtype=player.dtype)
        pile_cat = torch.cat(z["piles"], 1)
        dec = z["dec"]
        for r in range(self.rounds):
            ctx = self.ctx[r](torch.cat([player, (enemy * ep).sum(1), (hand * hp_).sum(1), (pot * pot_p).sum(1), zeros_c.index_copy(0, rows, ((cand * cand_p).sum(1) / 4.0).to(zeros_c.dtype)),
                                         pile_cat[:, :d], pile_cat[:, d:2 * d], pile_cat[:, 2 * d:], dec], 1))
            u = self.upd[r]
            player = player + u["player"](player.unsqueeze(1), ctx).squeeze(1)
            enemy = enemy + u["enemy"](enemy, ctx)
            hand = hand + u["hand"](hand, ctx)
            pot = pot + u["potion"](pot, ctx)
            if len(rows):
                cand = cand + u["cand"](cand, ctx[rows])
        gctx = torch.cat([player, ctx], 1)
        if _gctx:
            return gctx
        if not policy:
            return None, self.value(gctx).squeeze(-1)
        # targets: V[b, creature id] = v_tgt(enemy token); slot MAX_CREATURES = "no target"
        v = self.v_tgt(enemy) * ep
        V = torch.zeros(B, T + 1, d, device=obs.device, dtype=v.dtype)
        V[:, T] = self.v_none
        V = V.scatter_add(1, z["cid"].unsqueeze(-1).expand(-1, E, d), v)
        scale = 1.0 / math.sqrt(d)
        play = torch.einsum("bhd,btd->bht", self.u_card(hand), V) * scale + self.b_card(hand)
        pot_l = torch.einsum("bkd,btd->bkt", self.u_pot(pot), V) * scale + self.b_pot(pot)
        disc = self.disc_pot(pot).squeeze(-1)
        pick = torch.zeros(B, C["MAX_PICK"], device=obs.device)
        if len(rows):
            pv_ = self.pick(torch.cat([cand, dec[rows].unsqueeze(1).expand(-1, Q, -1)], -1)).squeeze(-1)
            pick = pick.index_copy(0, rows, F.pad(pv_, (0, C["MAX_PICK"] - Q)).to(pick.dtype))
        confirm = self.confirm(torch.cat([dec, player], 1))
        end = self.end(gctx)
        logits = torch.cat([end, play.flatten(1), pot_l.flatten(1), disc, pick, confirm], 1)
        # Clicking a selected card again would undo it: never useful, and it lets a greedy policy loop forever on a selection screen.
        # Removed from the legal set unless nothing else is legal.
        if len(rows):
            undo = torch.zeros(B, C["MAX_PICK"], dtype=torch.bool, device=obs.device)
            undo[rows, :Q] = z["cand_sel"]
            m = (mask > 0)
            m_pick = m[:, C["OFF_PICK"]:C["OFF_PICK"] + C["MAX_PICK"]] & ~undo
            m2 = torch.cat([m[:, :C["OFF_PICK"]], m_pick, m[:, C["OFF_PICK"] + C["MAX_PICK"]:]], 1)
            m = torch.where(m2.any(1, keepdim=True), m2, m)
        else:
            m = mask > 0
        logits = logits.masked_fill(~m, -1e9)
        return logits, (self.value(gctx).squeeze(-1) if value else None)


class Ensemble(nn.Module):
    """Several networks seen as one: the policy is the geometric mean of the members' policies (mean of log-probabilities), the value the mean of their values."""

    def __init__(self, nets):
        super().__init__()
        self.nets = nn.ModuleList(nets)

    def forward(self, obs, mask, policy=True, value=True, ufeat=None, **shape):
        outs = [n(obs, mask, policy=policy, value=value, ufeat=ufeat, **shape) for n in self.nets]
        lg = None
        if policy:
            lg = torch.stack([F.log_softmax(o[0], 1) for o in outs]).mean(0)
        v = torch.stack([o[1] for o in outs]).mean(0) if value else None
        return lg, v


def n_params(m):
    return sum(p.numel() for p in m.parameters())


def load(path):
    """A checkpoint, or several joined by commas (an `Ensemble`: mean policy log-probabilities, mean value), on `DEV` in eval mode."""
    if isinstance(path, (list, tuple)) or "," in path:
        parts = list(path) if isinstance(path, (list, tuple)) else path.split(",")
        return Ensemble([load(p) for p in parts]).to(DEV).eval()
    ck = torch.load(path, map_location="cpu")
    args = ck.get("args", {})
    net = Net(d=args.get("d", 64), rounds=args.get("rounds", 2))
    load_weights(net, ck["net"] if "net" in ck else ck)
    return net.to(DEV).eval()


def load_weights(net, sd):
    """A state dict into `net`; a checkpoint from before the HP-worth input (`ucond`) loads with that input at zero (identical behaviour)."""
    missing, unexpected = net.load_state_dict(sd, strict=False)
    bad = [k for k in missing if not k.startswith("ucond.")] + list(unexpected)
    if bad:
        raise RuntimeError(f"checkpoint does not match the network: {bad[:6]}")
