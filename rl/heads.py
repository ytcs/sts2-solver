"""The fight-outcome head (`docs/rl_redesign.md`, M1): a categorical distribution over how the fight ends, in absolute HP.

  class 0          a loss (death, or still fighting after TURN_CAP player turns)
  class b = 1..NB  a win with end HP in ((b - 1) * BIN, b * BIN]; the last class is open (end HP > (NB - 1) * BIN)

P(win) = 1 - P(class 0); HP lost if won = HP now - end HP. End HP does not change along a trajectory, so a state's training target is the next
state's predicted distribution (lambda-mixed with the one-hot ending), and any utility of the ending is one dot product (`value`).
With today's weights (`U(0) = -1`, `U(b) = 1 + 0.5 * end HP / max HP`) the value equals the scalar return the solver has always used, up to the
bin width: search, `Solver` and the value ensemble work unchanged.
"""
import numpy as np
import torch

TURN_CAP = 99  # player turns; a fight still running after that is a loss (env `turn_cap`, search `turn_cap`)
BIN = 2
NB = 75
NC = NB + 1
WIN, LOSS, HP_BONUS = 1.0, -1.0, 0.5


def end_class(won, hp_end):
    """Class of a finished fight: 0 for a loss, else the bin of the (absolute) end HP. Arrays or scalars."""
    b = np.clip(np.ceil(np.asarray(hp_end, np.float64) / BIN), 1, NB).astype(np.int64)
    return np.where(np.asarray(won, bool), b, 0)


def centers(device=None):
    """End HP at the centre of each win class 1..NB ([NB]); the open last class uses its lower edge plus one bin."""
    return (torch.arange(1, NB + 1, dtype=torch.float32, device=device) * BIN - (BIN - 1) / 2.0)


def utility(max_hp, win=WIN, loss=LOSS, hp_bonus=HP_BONUS):
    """[B, NC] worth of each class for rows with these max HPs (today's linear return)."""
    mx = max_hp.float().clamp(min=1).unsqueeze(1)
    u_win = win + hp_bonus * (centers(max_hp.device).unsqueeze(0) / mx).clamp(max=1.0)
    return torch.cat([torch.full_like(mx, loss), u_win], 1)


def value(logits, max_hp):
    """Expected worth [B] of the distribution `logits` [B, NC], in fp32 (a 76-way softmax in bf16 moves the value by more than a bin)."""
    p = torch.softmax(logits.float(), 1)
    return (p * utility(max_hp)).sum(1)
