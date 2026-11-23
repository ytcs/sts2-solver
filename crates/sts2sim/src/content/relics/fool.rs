use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(SongBow {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        cx.apply_power(ids::power::AIM_POWER, PLAYER, Dec::int(g::song_bow::AIM_POWER as i64), PLAYER, NO);
    }
});
