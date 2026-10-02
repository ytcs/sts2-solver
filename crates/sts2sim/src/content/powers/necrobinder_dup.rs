//! DUPLICATE stand-ins: powers other teammates also port (the silent / defect / regent branches each define
//! `EnergyNextTurnPower`). Keep exactly one definition when merging: delete this file.

use crate::hooks::*;
use crate::listener;
use crate::state::*;

listener!(EnergyNextTurnPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let n = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(me.amount, |p| p.amount);
        cx.gain_energy(n);
        cx.remove_power(me.owner, me.idx);
    }
});
