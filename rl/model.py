"""Policy / value network for the STS2 combat environment.

Reads the flat observation of `sts2.VecEnv` through `sts2.layout()` and builds one token per entity: the player (with relics,
powers, orbs, Osty, global state), every enemy (with its intent, move history and the expert look-ahead of its upcoming turns),
every hand card, every potion, every candidate of a pending card selection and the three pile multisets. A small transformer mixes
the tokens; the action head is a *pointer* head that scores exactly the dense action space of the env (play card x target, use
potion x target, discard potion, pick, confirm, end turn), so a card is scored from its own token, never from a fixed input slot.
"""
import math
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

import sts2

LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}


def sl(obs, name):
    o, s = SEC[name]
    return obs[:, o:o + s]


def mlp(i, h, o):
    return nn.Sequential(nn.Linear(i, h), nn.GELU(), nn.Linear(h, o))


class PowerPool(nn.Module):
    """Sum over a creature's powers of emb(id) gated by a function of the amount -> vector."""

    def __init__(self, n_powers, e):
        super().__init__()
        self.emb = nn.Embedding(n_powers + 1, e, padding_idx=0)
        self.amt = nn.Linear(4, e)
        self.out = nn.Linear(e, e)

    def forward(self, pw):  # pw [..., P, 2] (id+1, amount)
        pid = pw[..., 0].long().clamp(0, self.emb.num_embeddings - 1)
        a = pw[..., 1]
        f = torch.stack([a / 10.0, torch.sign(a) * torch.log1p(a.abs()) / 2.0, (a > 0).float(), (a < 0).float()], -1)
        v = self.emb(pid) * torch.tanh(self.amt(f) + 1.0)
        v = v * (pid > 0).unsqueeze(-1)
        return self.out(v.sum(-2))


class CardEnc(nn.Module):
    """Hand / candidate card -> d."""

    def __init__(self, d, e):
        super().__init__()
        self.card = nn.Embedding(C["N_CARDS"] + 1, e, padding_idx=0)
        self.upg = nn.Embedding(4, 8)
        self.ench = nn.Embedding(C["N_ENCHANTMENTS"] + 1, 8)
        self.aff = nn.Embedding(C["N_AFFLICTIONS"] + 1, 8)
        self.net = mlp(e + 24 + 8 + 12, 2 * d, d)

    def forward(self, f, extra):
        """f [..., 12] = id+1, upgrade, cost, playable, keywords, enchant, dmg, blk, c0, c1, ench_amt, afflic; extra [..., 2] = star cost, osty dmg."""
        cid = f[..., 0].long().clamp(0, self.card.num_embeddings - 1)
        kw = f[..., 4].long().unsqueeze(-1)
        bits = ((kw >> torch.arange(8, device=f.device)) & 1).float()
        num = torch.stack([f[..., 2] / 3.0, f[..., 3], f[..., 6] / 20.0, f[..., 7] / 20.0, f[..., 8] / 10.0, f[..., 9] / 10.0,
                           f[..., 10] / 5.0, (f[..., 0] > 0).float(), extra[..., 0] / 3.0, extra[..., 1] / 20.0, (f[..., 2] < 0).float(),
                           torch.sign(f[..., 6])], -1)
        x = torch.cat([self.card(cid), self.upg(f[..., 1].long().clamp(0, 3)), self.ench(f[..., 5].long().clamp(0, self.ench.num_embeddings - 1)),
                       self.aff(f[..., 11].long().clamp(0, self.aff.num_embeddings - 1)), bits, num], -1)
        return self.net(x)


class Net(nn.Module):
    def __init__(self, d=64, e=32, layers=1, heads=4):
        super().__init__()
        self.d = d
        P = C["OBS_POWERS"]
        self.pp = PowerPool(C["N_POWERS"], e)
        self.card = CardEnc(d, e)
        self.mon = nn.Embedding(C["N_MONSTERS"] + 1, e, padding_idx=0)
        self.kind = nn.Embedding(16, 8)
        self.node = nn.Embedding(C["LOOK_NODES"] + 8, 8, padding_idx=0)
        n_enemy_in = e + e + 7 + 3 * (8 + 3) + 4 * 8 + C["LOOK_H"] * (C["LOOK_NODES"] + 1)
        self.enemy = mlp(n_enemy_in, 2 * d, d)
        self.relic = nn.Embedding(C["N_RELICS"] + 1, e, padding_idx=0)
        self.relic_c = nn.Linear(1, e)
        self.potion = nn.Embedding(C["N_POTIONS"] + 1, e, padding_idx=0)
        self.potion_enc = mlp(e + 1, d, d)
        self.orb = nn.Embedding(C["N_ORBS"] + 2, 8, padding_idx=0)
        self.pile_card = nn.Embedding(C["N_CARDS"] + 1, e, padding_idx=0)
        self.pile_up = nn.Embedding(4, e)
        self.pile_enc = nn.ModuleList([mlp(e + 1, d, d) for _ in range(3)])
        n_player_in = 8 + 5 + 3 + 3 * e + C["MAX_ORBS"] * 8 + C["MAX_ORBS"] * 2 + 1 + 4 + e
        self.player = mlp(n_player_in, 2 * d, d)
        self.dec_src = nn.Embedding(10, 8)
        self.dec = mlp(8 + 7, d, d)
        # token types / positions
        self.n_tok = 1 + C["OBS_MAX_ENEMIES"] + C["MAX_HAND"] + C["MAX_POTIONS"] + C["OBS_MAX_CANDS"] + 3 + 1
        self.pos = nn.Parameter(torch.zeros(self.n_tok, d))
        nn.init.normal_(self.pos, std=0.02)
        layer = nn.TransformerEncoderLayer(d, heads, 2 * d, dropout=0.0, batch_first=True, norm_first=True, activation="gelu")
        self.tf = nn.TransformerEncoder(layer, layers, enable_nested_tensor=False)
        self.norm = nn.LayerNorm(d)
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

    # ------------------------------------------------------------------------------------------------------------
    def tokens(self, obs):
        B = obs.shape[0]
        d = self.d
        H, E, K, Q = C["MAX_HAND"], C["OBS_MAX_ENEMIES"], C["MAX_POTIONS"], C["OBS_MAX_CANDS"]
        P = C["OBS_POWERS"]
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
        look = sl(obs, "look").view(B, E, C["LOOK_H"] * (C["LOOK_NODES"] + 1))
        # ---- player ----
        stage = g[:, 2:5]
        gsc = torch.stack([g[:, 0] / 10.0, g[:, 1] / 10.0, g[:, 6] / 10.0, g[:, 7] / 5.0, g[:, 8] / 5.0], -1)
        sc = torch.stack([pl[:, 0] / 100.0, pl[:, 0] / pl[:, 1].clamp(min=1), pl[:, 2] / 50.0, pl[:, 3] / 5.0, pl[:, 4] / 5.0, pl[:, 5] / 5.0,
                          pl[:, 6] / 10.0, pl[:, 7] / 4.0], -1)
        ppow = self.pp(pl[:, 8:8 + 2 * P].view(B, P, 2))
        rid = relics[..., 0].long().clamp(0, self.relic.num_embeddings - 1)
        rb = ((self.relic(rid) * torch.tanh(self.relic_c(relics[..., 1:2] / 10.0) + 1.0)) * (rid > 0).unsqueeze(-1)).sum(1)
        pid = potions[..., 0].long().clamp(0, self.potion.num_embeddings - 1)
        pb = (self.potion(pid) * (pid > 0).unsqueeze(-1)).sum(1)
        orb_kind = orbs[:, :C["MAX_ORBS"] * 3].view(B, C["MAX_ORBS"], 3)
        orb_e = self.orb(orb_kind[..., 0].long().clamp(0, self.orb.num_embeddings - 1)).flatten(1)
        orb_v = torch.cat([orb_kind[..., 1:3].flatten(1) / 10.0, orbs[:, -1:] / 5.0], 1)
        osty_f = torch.cat([osty[:, :4] / torch.tensor([1.0, 1.0, 50.0, 50.0], device=obs.device), self.pp(osty[:, 4:4 + 2 * P].view(B, P, 2))], 1)
        pin = torch.cat([sc, gsc, stage, ppow, rb, pb, orb_e, orb_v, osty_f], 1)
        player = self.player(pin)
        # ---- enemies ----
        ep = enemies[..., 0] > 0.5
        mon = self.mon(enemies[..., 2].long().clamp(0, self.mon.num_embeddings - 1))
        epow = self.pp(enemies[..., 8:8 + 2 * P].view(B, E, P, 2))
        esc = torch.stack([enemies[..., 3] / 100.0, enemies[..., 3] / enemies[..., 4].clamp(min=1), enemies[..., 5] / 50.0, enemies[..., 4] / 200.0,
                           enemies[..., 6], enemies[..., 7], ep.float()], -1)
        iv = enemies[..., 8 + 2 * P:8 + 2 * P + 9].view(B, E, 3, 3)
        ie = self.kind(iv[..., 0].long().clamp(0, 15))
        inum = torch.stack([iv[..., 1] / 30.0, iv[..., 2] / 5.0, iv[..., 1] * iv[..., 2] / 60.0], -1)
        perf = enemies[..., 8 + 2 * P + 9:8 + 2 * P + 13].long().clamp(0, self.node.num_embeddings - 1)
        pe = self.node(perf).flatten(2)
        lk = look.clone()
        # expected damage columns are scaled down
        lk = lk.view(B, E, C["LOOK_H"], C["LOOK_NODES"] + 1)
        lk = torch.cat([lk[..., :-1], lk[..., -1:] / 50.0], -1).flatten(2)
        ein = torch.cat([mon, epow, esc, torch.cat([ie, inum], -1).flatten(2), pe, lk], -1)
        enemy = self.enemy(ein)
        cid = enemies[..., 1].long().clamp(0, C["MAX_CREATURES"] - 1)
        # ---- hand ----
        hp_ = (hand[..., 0] > 0)
        hextra = torch.stack([regent[:, :H], osty[:, -H:]], -1)
        hand_t = self.card(hand, hextra)
        # ---- potions ----
        pot_t = self.potion_enc(torch.cat([self.potion(pid), potions[..., 1:2]], -1))
        pot_p = pid > 0
        # ---- decision candidates ----
        cands = dec[:, 8:].view(B, Q, C["CARD_F"] + 1)
        cextra = torch.stack([regent[:, H:H + Q], torch.zeros_like(regent[:, H:H + Q])], -1)
        cand_t = self.card(cands[..., :C["CARD_F"]], cextra) + 0.0
        sel = cands[..., C["CARD_F"]:C["CARD_F"] + 1]
        cand_t = cand_t + sel * self.pos.new_ones(1, 1, d) * 0.5
        cand_p = cands[..., 0] > 0
        dsrc = self.dec_src(dec[:, 1].long().clamp(0, 9))
        dh = torch.cat([dsrc, dec[:, 0:1], dec[:, 2:3] / 4.0, dec[:, 3:4] / 4.0, dec[:, 4:5] / 4.0, dec[:, 5:6], dec[:, 6:7], dec[:, 7:8] / 10.0], 1)
        dec_t = self.dec(dh)
        dec_p = dec[:, 0] > 0.5
        # ---- piles ----
        sizes = sl(obs, "pile_sizes")
        pile_t = []
        for k, nm in enumerate(["draw", "discard", "exhaust"]):
            pv = sl(obs, nm).view(B, -1, 2)
            ids = pv[..., 0].long().clamp(0, self.pile_card.num_embeddings - 1)
            v = (self.pile_card(ids) + self.pile_up(pv[..., 1].long().clamp(0, 3))) * (ids > 0).unsqueeze(-1)
            s = v.sum(1) / 4.0
            pile_t.append(self.pile_enc[k](torch.cat([s, sizes[:, k:k + 1] / 20.0], 1)))
        pile_t = torch.stack(pile_t, 1)
        ones = torch.ones(B, 1, dtype=torch.bool, device=obs.device)
        tok = torch.cat([player.unsqueeze(1), enemy, hand_t, pot_t, cand_t, pile_t, dec_t.unsqueeze(1)], 1) + self.pos
        present = torch.cat([ones, ep, hp_, pot_p, cand_p, ones.expand(B, 3), dec_p.unsqueeze(1)], 1)
        return tok, present, cid, ep

    def forward(self, obs, mask):
        """Returns (masked logits [B, ACTION_SPACE], value [B])."""
        B = obs.shape[0]
        d = self.d
        H, E, K, Q = C["MAX_HAND"], C["OBS_MAX_ENEMIES"], C["MAX_POTIONS"], C["OBS_MAX_CANDS"]
        T = C["MAX_CREATURES"]
        tok, present, cid, ep = self.tokens(obs)
        x = self.tf(tok, src_key_padding_mask=~present)
        x = self.norm(x)
        i = 0
        ctx = x[:, 0]; i = 1
        en = x[:, i:i + E]; i += E
        hd = x[:, i:i + H]; i += H
        pt = x[:, i:i + K]; i += K
        cd = x[:, i:i + Q]; i += Q
        pl3 = x[:, i:i + 3]; i += 3
        dc = x[:, i]
        pooled = (x * present.unsqueeze(-1)).sum(1) / present.sum(1, keepdim=True).clamp(min=1)
        gctx = torch.cat([ctx, pooled], 1)
        # targets: V[b, creature id] = v_tgt(enemy token); slot MAX_CREATURES = "no target"
        v = self.v_tgt(en) * ep.unsqueeze(-1)
        V = torch.zeros(B, T + 1, d, device=obs.device, dtype=v.dtype)
        V[:, T] = self.v_none
        V = V.scatter_add(1, cid.unsqueeze(-1).expand(-1, E, d), v)
        scale = 1.0 / math.sqrt(d)
        u = self.u_card(hd)
        play = torch.einsum("bhd,btd->bht", u, V) * scale + self.b_card(hd)
        up = self.u_pot(pt)
        pot = torch.einsum("bkd,btd->bkt", up, V) * scale + self.b_pot(pt)
        disc = self.disc_pot(pt).squeeze(-1)
        pick_vis = self.pick(torch.cat([cd, dc.unsqueeze(1).expand(-1, Q, -1)], -1)).squeeze(-1)
        pick = F.pad(pick_vis, (0, C["MAX_PICK"] - Q))
        confirm = self.confirm(torch.cat([dc, ctx], 1))
        end = self.end(gctx)
        logits = torch.cat([end, play.flatten(1), pot.flatten(1), disc, pick, confirm], 1)
        logits = logits.masked_fill(mask == 0, -1e9)
        value = self.value(gctx).squeeze(-1)
        return logits, value


def n_params(m):
    return sum(p.numel() for p in m.parameters())
