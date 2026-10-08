import numpy as np
import torch

TURN_CAP = 99
BIN = 2
NB = 75
NC = NB + 1
WIN, LOSS, HP_BONUS = 1.0, -1.0, 0.5


def end_class(won, hp_end):
    b = np.clip(np.ceil(np.asarray(hp_end, np.float64) / BIN), 1, NB).astype(np.int64)
    return np.where(np.asarray(won, bool), b, 0)


def centers(device=None):
    return (torch.arange(1, NB + 1, dtype=torch.float32, device=device) * BIN - (BIN - 1) / 2.0)


def utility(max_hp, win=WIN, loss=LOSS, hp_bonus=HP_BONUS):
    mx = max_hp.float().clamp(min=1).unsqueeze(1)
    u_win = win + hp_bonus * (centers(max_hp.device).unsqueeze(0) / mx).clamp(max=1.0)
    return torch.cat([torch.full_like(mx, loss), u_win], 1)


def value(logits, max_hp):
    p = torch.softmax(logits.float(), 1)
    return (p * utility(max_hp)).sum(1)


CENTERS = centers().numpy()
CENTERS0 = np.concatenate([[0.0], CENTERS])
CENTERS.flags.writeable = CENTERS0.flags.writeable = False
