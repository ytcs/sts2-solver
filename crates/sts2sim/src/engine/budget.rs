use crate::state::*;

impl Combat {
    #[inline(always)]
    pub(crate) fn budget_reset(&mut self) {
        self.work = 0;
        self.hook_depth = 0;
        self.step_turns = 0;
    }

    #[inline(always)]
    pub(crate) fn tick(&mut self) -> bool {
        self.work += 1;
        if self.work > self.work_limit {
            self.trip_loop();
            return false;
        }
        true
    }

    #[inline(always)]
    pub(crate) fn pass_enter(&mut self) -> bool {
        let (w, d) = (self.work + 1, self.hook_depth);
        self.work = w;
        if (w > self.work_limit) | (d >= HOOK_DEPTH_LIMIT) {
            self.trip_loop();
            return false;
        }
        self.hook_depth = d + 1;
        true
    }

    #[inline(always)]
    pub(crate) fn pass_exit(&mut self) {
        self.hook_depth = self.hook_depth.wrapping_sub(1);
    }

    #[inline]
    pub(crate) fn turn_enter(&mut self) -> bool {
        self.step_turns += 1;
        if self.step_turns > TURN_LIMIT {
            self.trip_loop();
            return false;
        }
        true
    }

    #[cold]
    #[inline(never)]
    pub(crate) fn trip_loop(&mut self) {
        self.overflow |= ov::LOOP;
        self.work_limit = 0;
        self.in_progress = false;
        self.stage = Stage::Over;
    }
}
