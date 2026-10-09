import math
import os
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

import sts2
import heads as H

DEV = torch.device(os.environ.get("STS2_DEVICE", "cpu"))
LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}


def S(x):
    return torch.sign(x) * torch.log1p(x.abs())


def scale(x, div, rec):
    return x * rec if x.is_cuda else x / div


def sl(obs, name, sec=SEC):
    o, s = sec[name]
    return obs[:, o:o + s]


def mlp(i, h, o):
    return nn.Sequential(nn.Linear(i, h), nn.ReLU(), nn.Linear(h, o))


class CtxMLP(nn.Module):
    def __init__(self, d, h, o):
        super().__init__()
        self.a = nn.Linear(d, h)
        self.b = nn.Linear(d, h, bias=False)
        self.o = nn.Linear(h, o)

    def forward(self, x, ctx):
        return self.o(F.relu(self.a(x) + self.b(ctx).unsqueeze(1)))


class Bag(nn.Module):
    def __init__(self, n, e, channels):
        super().__init__()
        self.n, self.k = n + 1, channels
        self.emb = nn.Embedding(self.k * self.n, e, padding_idx=0)
        self.register_buffer("offs", torch.arange(self.k).view(1, 1, -1) * self.n, persistent=False)

    def forward(self, ids, w):
        lead = ids.shape[:-1]
        L = ids.shape[-1]
        ids = ids.reshape(-1, L)
        w = w.reshape(-1, L, self.k)
        full = (ids.unsqueeze(-1) + self.offs * (ids > 0).unsqueeze(-1)).reshape(-1, L * self.k)
        psw = w.reshape(-1, L * self.k) * (ids > 0).repeat_interleave(self.k, 1)
        if torch.is_grad_enabled() and self.emb.weight.requires_grad:
            # same sums in the same order without the padding entries: the backward's cost follows the index count
            keep = full > 0
            n = keep.sum(1)
            out = F.embedding_bag(full[keep], self.emb.weight, n.cumsum(0) - n, mode="sum", per_sample_weights=psw[keep], padding_idx=0)
        else:
            out = F.embedding_bag(full, self.emb.weight, per_sample_weights=psw, mode="sum", padding_idx=0)
        return out.view(*lead, -1)


class PowerPool(nn.Module):
    def __init__(self, n_powers, e):
        super().__init__()
        self.bag = Bag(n_powers, e, 4)

    def forward(self, pw, spw=None):
        pid = pw[..., 0].long().clamp(0, self.bag.n - 1)
        a = pw[..., 1]
        if spw is None:
            spw = S(pw)
        f = [torch.ones_like(a), spw[..., 1] / 2.0, a.clamp(-10, 10) / 10.0, spw[..., 2] / 2.0]
        return self.bag(pid, torch.stack(f, -1))


class CardEnc(nn.Module):
    def __init__(self, d, e):
        super().__init__()
        self.card = nn.Embedding(C["N_CARDS"] + 1, e, padding_idx=0)
        self.upg = nn.Embedding(4, 8)
        self.ench = nn.Embedding(C["N_ENCHANTMENTS"] + 1, 8)
        self.aff = nn.Embedding(C["N_AFFLICTIONS"] + 1, 8)
        self.net = mlp(e + 24 + 8 + 16, 2 * d, d)
        self.register_buffer("bit_idx", torch.arange(8), persistent=False)
        self.register_buffer("num_div", torch.tensor([1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0]), persistent=False)
        self.register_buffer("num_rec", self.num_div.reciprocal(), persistent=False)

    def forward(self, f, extra, sf=None, sextra=None):
        cid = f[..., 0].long().clamp(0, self.card.num_embeddings - 1)
        kw = f[..., 4].long().unsqueeze(-1)
        bits = ((kw >> self.bit_idx) & 1).float()
        if sf is None:
            sf = S(f)
        if sextra is None:
            sextra = S(extra[..., :2])
        parts = [sf[..., 2:3], f[..., 3:4], sf[..., 6:11], (f[..., 0:1] > 0).float(), sextra, (f[..., 2:3] < 0).float(), torch.sign(f[..., 6:7]), extra[..., 2:3],
                 sf[..., 12:15]]
        num = scale(torch.cat(parts, -1), self.num_div, self.num_rec)
        x = torch.cat([self.card(cid), self.upg(f[..., 1].long().clamp(0, 3)), self.ench(f[..., 5].long().clamp(0, self.ench.num_embeddings - 1)),
                       self.aff(f[..., 11].long().clamp(0, self.aff.num_embeddings - 1)), bits, num], -1)
        return self.net(x)


class Net(nn.Module):
    def __init__(self, d=64, e=24, rounds=2, heads=False):
        super().__init__()
        self.d = d
        self.C, self.SEC = C, SEC
        self.heads = heads
        self.rounds = rounds
        self.pp = PowerPool(C["N_POWERS"], e)
        self.card = CardEnc(d, e)
        self.mon = nn.Embedding(C["N_MONSTERS"] + 1, e, padding_idx=0)
        self.kind = nn.Embedding(16, 8)
        self.node = nn.Embedding(C["LOOK_NODES"] + 8, 8, padding_idx=0)
        n_enemy_in = e + e + 7 + 3 * (8 + 3) + 4 * 8 + C["LOOK_H"] * (C["LOOK_NODES"] + 1) + C["MOVE_STATE_F"] * 8
        self.enemy = mlp(n_enemy_in, d, d)
        self.relic = Bag(C["N_RELICS"], e, 2)
        self.potion = nn.Embedding(C["N_POTIONS"] + 1, e, padding_idx=0)
        self.potion_enc = mlp(e + 1, d, d)
        self.orb = nn.Embedding(C["N_ORBS"] + 2, 4, padding_idx=0)
        self.pile = Bag(C["N_CARDS"], e, 2)
        self.pile_enc = nn.ModuleList([mlp(e + 1, d, d) for _ in range(3)])
        n_player_in = 8 + 5 + 3 + 3 * e + C["MAX_ORBS"] * 4 + C["MAX_ORBS"] * 2 + 1 + 4 + e
        self.player = mlp(n_player_in, 2 * d, d)
        self.dec_src = nn.Embedding(10, 8)
        base = [0, 0, C["N_CARDS"], C["N_CARDS"] + C["N_POTIONS"], C["N_CARDS"] + C["N_POTIONS"] + C["N_RELICS"]]
        self.register_buffer("src_base", torch.tensor(base), persistent=False)
        self.src_id = nn.Embedding(base[-1] + C["N_MONSTERS"] + 1, 8, padding_idx=0)
        self.played = nn.Linear(d, d)
        self.dec = mlp(8 + 7 + 8, d, d)
        self.ctx = nn.ModuleList([mlp(d * 9, d, d) for _ in range(rounds)])
        self.upd = nn.ModuleList([nn.ModuleDict({k: CtxMLP(d, d, d) for k in ("player", "enemy", "hand", "potion", "cand")}) for _ in range(rounds)])
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
        if heads:
            self.outcome = mlp(2 * d, 2 * d, H.NC)
        # trained checkpoints carry non-zero ucond weights: ucond(lin_feats) is a constant input
        self.ucond = nn.Linear(8, d)
        nn.init.zeros_(self.ucond.weight)
        nn.init.zeros_(self.ucond.bias)
        self.register_buffer("lin_feats", torch.tensor([0.12, 0.25, 0.37, 0.50, 0.62, 0.75, 0.87, 1.00]), persistent=False)
        self.register_buffer("sc_div", torch.tensor([3.0, 1.0, 3.0, 1.0, 1.0, 1.0, 1.0, 1.0]), persistent=False)
        self.register_buffer("esc_div", torch.tensor([3.0, 1.0, 3.0, 3.0, 1.0, 1.0, 1.0]), persistent=False)
        self.register_buffer("inum_div", torch.tensor([2.0, 1.0]), persistent=False)
        for k in ("sc", "esc", "inum"):
            self.register_buffer(f"{k}_rec", getattr(self, f"{k}_div").reciprocal(), persistent=False)

    def encode(self, obs, E=None, L=None, has_dec=None, rows=None, rows_w=None):
        B = obs.shape[0]
        C, SEC = self.C, self.SEC
        sl = lambda o, name: o[:, SEC[name][0]:SEC[name][0] + SEC[name][1]]  # noqa: E731
        H, E, K, Q = C["MAX_HAND"], C["OBS_MAX_ENEMIES"], C["MAX_POTIONS"], C["OBS_MAX_CANDS"]
        P, PF = C["OBS_POWERS"], C["POWER_F"]
        dev = obs.device
        So = S(obs)
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
        moves = sl(obs, "enemy_moves").view(B, E, C["MOVE_STATE_F"])
        if E is None:
            E = max(1, int((enemies[..., 0] > 0.5).any(0).nonzero().max().item() + 1)) if (enemies[..., 0] > 0.5).any() else 1
        enemies, look, moves = enemies[:, :E], look[:, :E], moves[:, :E]
        stage = g[:, 2:5]
        sg, spl, senemies, sosty, sorbs = sl(So, "global"), sl(So, "player"), sl(So, "enemies").view(B, -1, C["ENEMY_F"])[:, :E], sl(So, "osty"), sl(So, "orbs")
        gsc = torch.cat([sg[:, 0:2], sg[:, 6:9]], 1) / 2.0
        sc = scale(torch.cat([spl[:, 0:1], (pl[:, 0] / pl[:, 1].clamp(min=1)).unsqueeze(1), spl[:, 2:8]], 1), self.sc_div, self.sc_rec)
        pw = torch.cat([pl[:, 8:8 + PF * P].view(B, 1, P, PF), enemies[..., 8:8 + PF * P].reshape(B, E, P, PF), osty[:, 4:4 + PF * P].view(B, 1, P, PF)], 1)
        spw = torch.cat([spl[:, 8:8 + PF * P].view(B, 1, P, PF), senemies[..., 8:8 + PF * P].reshape(B, E, P, PF), sosty[:, 4:4 + PF * P].view(B, 1, P, PF)], 1)
        pv = self.pp(pw, spw)
        ppow, epow, opow = pv[:, 0], pv[:, 1:1 + E], pv[:, -1]
        rid = relics[..., 0].long().clamp(0, C["N_RELICS"])
        rb = self.relic(rid, torch.stack([torch.ones_like(relics[..., 1]), sl(So, "relics").view(B, -1, 2)[..., 1] / 2.0], -1))
        pid = potions[..., 0].long().clamp(0, C["N_POTIONS"])
        pb = self.potion(pid).sum(1)
        orb_kind = orbs[:, :C["MAX_ORBS"] * 3].view(B, C["MAX_ORBS"], 3)
        orb_e = self.orb(orb_kind[..., 0].long().clamp(0, C["N_ORBS"] + 1)).flatten(1)
        orb_v = torch.cat([sorbs[:, :C["MAX_ORBS"] * 3].view(B, C["MAX_ORBS"], 3)[..., 1:3].flatten(1) / 2.0, sorbs[:, -1:]], 1)
        osty_f = torch.cat([osty[:, :2], sosty[:, 2:4] / 3.0, opow], 1)
        player = self.player(torch.cat([sc, gsc, stage, ppow, rb, pb, orb_e, orb_v, osty_f], 1))
        ep = enemies[..., 0] > 0.5
        mon = self.mon(enemies[..., 2].long().clamp(0, C["N_MONSTERS"]))
        esc = scale(torch.cat([senemies[..., 3:4], (enemies[..., 3] / enemies[..., 4].clamp(min=1)).unsqueeze(-1), senemies[..., 5:6], senemies[..., 4:5],
                         enemies[..., 6:8], ep.float().unsqueeze(-1)], -1), self.esc_div, self.esc_rec)
        iv = enemies[..., 8 + PF * P:8 + PF * P + 9].reshape(B, E, 3, 3)
        siv = senemies[..., 8 + PF * P:8 + PF * P + 9].reshape(B, E, 3, 3)
        ie = self.kind(iv[..., 0].long().clamp(0, 15))
        inum = torch.cat([scale(siv[..., 1:3], self.inum_div, self.inum_rec), (S(iv[..., 1] * iv[..., 2]) / 3.0).unsqueeze(-1)], -1)
        perf = enemies[..., 8 + PF * P + 9:8 + PF * P + 13].long().clamp(0, C["LOOK_NODES"] + 7)
        pe = self.node(perf).flatten(2)
        slook = sl(So, "look").view(B, C["OBS_MAX_ENEMIES"], C["LOOK_H"], C["LOOK_NODES"] + 1)[:, :E]
        lk = torch.cat([look[..., :-1], slook[..., -1:] / 3.0], -1).flatten(2)
        me = self.node(moves.long().clamp(0, C["LOOK_NODES"] + 7)).flatten(2)
        enemy = self.enemy(torch.cat([mon, epow, esc, torch.cat([ie, inum], -1).flatten(2), pe, lk, me], -1))
        cid = enemies[..., 1].long().clamp(0, C["MAX_CREATURES"] - 1)
        hp_ = hand[..., 0] > 0
        sregent = sl(So, "regent")
        hand_t = self.card(hand, torch.stack([regent[:, :H], osty[:, -H:], torch.zeros_like(osty[:, -H:])], -1), sl(So, "hand").view(B, H, C["CARD_F"]),
                           torch.stack([sregent[:, :H], sosty[:, -H:]], -1))
        pot_t = self.potion_enc(torch.cat([self.potion(pid), potions[..., 1:2]], -1))
        pot_p = pid > 0
        if rows is not None:
            pass
        elif has_dec is None:
            rows = (dec[:, 0] > 0.5).nonzero().squeeze(1)
        else:
            rows = torch.arange(B, device=dev) if has_dec else torch.zeros(0, dtype=torch.long, device=dev)
        cands = dec[rows, 8:].view(len(rows), Q, C["CARD_F"] + 1)
        rg = regent[rows, H:H + Q]
        sdec = sl(So, "decision")
        scands = sdec[rows, 8:].view(len(rows), Q, C["CARD_F"] + 1)
        srg = sregent[rows, H:H + Q]
        cand_t = self.card(cands[..., :C["CARD_F"]], torch.stack([rg, torch.zeros_like(rg), cands[..., C["CARD_F"]]], -1), scands[..., :C["CARD_F"]],
                           torch.stack([srg, torch.zeros_like(srg)], -1))
        cand_p = cands[..., 0] > 0
        src = sl(obs, "dec_source")
        kind = src[:, 0].long().clamp(0, 4)
        dh = [self.dec_src(dec[:, 1].long().clamp(0, 9)), dec[:, 0:1], sdec[:, 2:5], dec[:, 5:7], sdec[:, 7:8],
              self.src_id(((self.src_base[kind] + src[:, 1].long()) * (kind > 0)).clamp(0, self.src_id.num_embeddings - 1))]
        dec_t = self.dec(torch.cat(dh, 1))
        pc = sl(obs, "played")
        spc = sl(So, "played")
        CF = C["CARD_F"]
        pt = self.card(pc[:, None, :CF], torch.stack([pc[:, CF], pc[:, CF + 1], torch.zeros_like(pc[:, CF])], -1)[:, None], spc[:, None, :CF],
                       spc[:, None, CF:CF + 2]).squeeze(1)
        dec_t = dec_t + self.played(pt) * (pc[:, :1] > 0).to(pt.dtype)
        sizes = sl(So, "pile_sizes") / 3.0
        piles = []
        for k, nm in enumerate(["draw", "discard", "exhaust"]):
            pv_ = sl(obs, nm).view(B, -1, 2)
            Lk = (L[k] if isinstance(L, (tuple, list)) else L) if L is not None else max(1, int((pv_[..., 0] > 0).sum(1).max().item()))
            pv_ = pv_[:, :Lk]
            ids = pv_[..., 0].long().clamp(0, C["N_CARDS"])
            up = (pv_[..., 1] > 0).float()
            w = torch.stack([1.0 - up, up], -1) / 4.0
            piles.append(self.pile_enc[k](torch.cat([self.pile(ids, w), sizes[:, k:k + 1]], 1)))
        return dict(player=player, enemy=enemy, hand=hand_t, potion=pot_t, cand=cand_t, piles=piles, dec=dec_t, ep=ep, hp=hp_, pot_p=pot_p,
                    cand_p=cand_p, cid=cid, rows=rows, rows_w=rows_w, cand_sel=cands[..., C["CARD_F"]] > 0.5)

    def heads_out(self, obs, **shape):
        return self.forward(obs, None, policy=False, value=False, _heads=True, **shape)

    def forward(self, obs, mask, policy=True, value=True, outcome=False, _heads=False, **shape):
        B = obs.shape[0]
        d = self.d
        C = self.C
        if obs.shape[1] != C["OBS_SIZE"]:
            raise ValueError(f"observation rows of {obs.shape[1]} floats, want {C['OBS_SIZE']}")
        E, Q = C["OBS_MAX_ENEMIES"], C["OBS_MAX_CANDS"]
        T = C["MAX_CREATURES"]
        z = self.encode(obs, **shape)
        player, enemy, hand, pot, cand = z["player"], z["enemy"], z["hand"], z["potion"], z["cand"]
        player = player + self.ucond(self.lin_feats.to(player.dtype).expand(B, 8))
        ep, hp_, pot_p, cand_p = z["ep"].unsqueeze(-1), z["hp"].unsqueeze(-1), z["pot_p"].unsqueeze(-1), z["cand_p"].unsqueeze(-1)
        rows, rw = z["rows"], z["rows_w"]
        if rw is None:
            put = lambda base, x: base.index_copy(0, rows, x.to(base.dtype))  # noqa: E731
        else:
            # fixed-size row list padded with weight-0 entries (graphs): the padding adds exact zeros
            put = lambda base, x: base.index_add(0, rows, (x * rw.view(-1, *[1] * (x.dim() - 1)).to(x.dtype)).to(base.dtype))  # noqa: E731
        E = enemy.shape[1]
        zeros_c = torch.zeros(B, d, device=obs.device, dtype=player.dtype)
        pile_cat = torch.cat(z["piles"], 1)
        dec = z["dec"]
        for r in range(self.rounds):
            ctx = self.ctx[r](torch.cat([player, (enemy * ep).sum(1), (hand * hp_).sum(1), (pot * pot_p).sum(1), put(zeros_c, (cand * cand_p).sum(1) / 4.0),
                                         pile_cat[:, :d], pile_cat[:, d:2 * d], pile_cat[:, 2 * d:], dec], 1))
            u = self.upd[r]
            player = player + u["player"](player.unsqueeze(1), ctx).squeeze(1)
            enemy = enemy + u["enemy"](enemy, ctx)
            hand = hand + u["hand"](hand, ctx)
            pot = pot + u["potion"](pot, ctx)
            if len(rows):
                cand = cand + u["cand"](cand, ctx[rows])
        gctx = torch.cat([player, ctx], 1)
        if _heads:
            return self.outcome_logits(gctx)
        if not policy:
            return None, self._value(gctx, obs)
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
            pick = put(pick, F.pad(pv_, (0, C["MAX_PICK"] - Q)))
        confirm = self.confirm(torch.cat([dec, player], 1))
        end = self.end(gctx)
        logits = torch.cat([end, play.flatten(1), pot_l.flatten(1), disc, pick, confirm], 1)
        if len(rows):
            if rw is None:
                undo = torch.zeros(B, C["MAX_PICK"], dtype=torch.bool, device=obs.device)
                undo[rows, :Q] = z["cand_sel"]
            else:
                undo = put(torch.zeros(B, C["MAX_PICK"], device=obs.device), F.pad(z["cand_sel"].float(), (0, C["MAX_PICK"] - Q))) > 0
            m = (mask > 0)
            m_pick = m[:, C["OFF_PICK"]:C["OFF_PICK"] + C["MAX_PICK"]] & ~undo
            m2 = torch.cat([m[:, :C["OFF_PICK"]], m_pick, m[:, C["OFF_PICK"] + C["MAX_PICK"]:]], 1)
            m = torch.where(m2.any(1, keepdim=True), m2, m)
        else:
            m = mask > 0
        logits = logits.masked_fill(~m, -1e9)
        if outcome:
            ol = self.outcome_logits(gctx)
            return logits, H.value(ol, sl(obs, "player", self.SEC)[:, 1]), ol
        return logits, (self._value(gctx, obs) if value else None)

    def outcome_logits(self, gctx):
        with torch.autocast(gctx.device.type, enabled=False):
            return self.outcome(gctx.float())

    def _value(self, gctx, obs):
        if not self.heads:
            return self.value(gctx).squeeze(-1)
        return H.value(self.outcome_logits(gctx), sl(obs, "player", self.SEC)[:, 1])


class HostShape:
    @staticmethod
    def rows_info(obs):
        o, s = SEC["enemies"]
        occ = obs[:, o:o + s].reshape(len(obs), C["OBS_MAX_ENEMIES"], C["ENEMY_F"])[..., 0] > 0.5
        e = np.where(occ.any(1), C["OBS_MAX_ENEMIES"] - np.argmax(occ[:, ::-1], 1), 0).astype(np.int16)
        piles = np.stack([(obs[:, SEC[nm][0]:SEC[nm][0] + SEC[nm][1]:2] > 0).sum(1) for nm in ("draw", "discard", "exhaust")], 1).astype(np.int16)
        dec = obs[:, SEC["decision"][0]] > 0.5
        return e, piles, dec

    @staticmethod
    def of(e, piles, dec, device):
        return dict(E=max(1, int(e.max())), L=tuple(max(1, int(x)) for x in piles.max(0)),
                    rows=torch.from_numpy(np.flatnonzero(dec)).to(device, non_blocking=True))


def n_params(m):
    return sum(p.numel() for p in m.parameters())


def load(path):
    ck = torch.load(path, map_location="cpu")
    args = ck.get("args", {})
    net = Net(d=args.get("d", 64), rounds=args.get("rounds", 2), heads=bool(args.get("heads", False)))
    load_weights(net, ck["net"] if "net" in ck else ck)
    return net.to(DEV).eval()


def net_policy(net, greedy=True):
    @torch.no_grad()
    def act(obs, mask):
        lg, _ = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV))
        a = lg.argmax(1) if greedy else torch.distributions.Categorical(logits=lg).sample()
        return a.cpu().numpy().astype(np.int32)
    return act


def load_weights(net, sd, allow_missing=()):
    missing, unexpected = net.load_state_dict(sd, strict=False)
    bad = [k for k in missing if not k.startswith(tuple(allow_missing))] + list(unexpected)
    if bad:
        raise RuntimeError(f"checkpoint does not match the network: {bad[:6]}")
