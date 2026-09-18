//! The hero's animation counter, porting the block that drives it in
//! `main.c`'s loop.
//!
//! It is here rather than inline in the loop because getting it wrong is
//! invisible to a renderer test: `map_render` is handed a tick and draws the
//! frame it names, so a tick advancing sixty times a second draws a correct
//! frame every time and still flickers on screen.

/// Where the free-running counter wraps, C's `OB_ANIM_TICK_WRAP`.
///
/// 720720 is divisible by every integer from 1 to 16, which is the longest
/// animation a pack may declare. So whatever cycle a strip folds this onto,
/// the wrap lands back on frame 0 and nothing stutters as the counter rolls.
pub const ANIM_TICK_WRAP: i32 = 720_720;

/// How long after a step the hero keeps walking, in seconds.
pub const MOVE_WINDOW: f64 = 0.4;

/// Delay option: 0.05s per step of `options[0]`, on top of a 0.05s floor.
pub const DELAY_BASE: f64 = 0.05;
pub const DELAY_STEP: f64 = 0.05;

/// `stats.options` slots this reads. The pack's `controls.settings` declares
/// them in this order.
pub const OPTION_DELAY: usize = 0;
pub const OPTION_ANIMATION: usize = 3;

/// The hero's animation state, advanced on its own clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeroAnimator {
    tick: i32,
    moving: bool,
    next_at: f64,
}

impl HeroAnimator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The free-running counter a sprite strip folds onto its own cycle.
    pub fn tick(&self) -> i32 {
        self.tick
    }

    /// Whether the hero counts as mid-step, which is what chooses the walk
    /// set over the idle one.
    pub fn moving(&self) -> bool {
        self.moving
    }

    /// Advance the counter if its interval has elapsed.
    ///
    /// `last_step` is when the hero last moved, or `None` if he never has.
    /// `has_idle` is whether the pack shipped an idle animation.
    ///
    /// Two rules do the work, and both matter on screen:
    ///
    /// - The counter moves on ITS OWN clock, every 0.05 to 0.30 seconds by
    ///   the delay option, not once per frame. Advancing it per frame runs a
    ///   four-frame walk cycle fifteen times a second, which reads as a
    ///   flicker rather than a walk.
    /// - A hero who is not mid-step snaps back to frame 0 unless the pack
    ///   ships an idle animation, in which case the counter keeps running so
    ///   he breathes instead of freezing. The shipped pack ships none, so a
    ///   standing hero is perfectly still.
    pub fn update(&mut self, now: f64, last_step: Option<f64>, options: &[i32], has_idle: bool) {
        if now < self.next_at {
            return;
        }
        let enabled = options.get(OPTION_ANIMATION).copied().unwrap_or(0) != 0;
        let stepping = last_step.is_some_and(|at| now - at < MOVE_WINDOW);
        self.moving = enabled && stepping;

        if self.moving || has_idle {
            self.tick = (self.tick + 1) % ANIM_TICK_WRAP;
        } else {
            self.tick = 0;
        }

        let delay = options.get(OPTION_DELAY).copied().unwrap_or(0).max(0);
        self.next_at = now + DELAY_BASE + f64::from(delay) * DELAY_STEP;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped pack's defaults: delay 4, animation on.
    const OPTIONS: [i32; 5] = [4, 1, 1, 1, 1];
    /// 0.05 + 4 * 0.05.
    const INTERVAL: f64 = 0.25;

    #[test]
    fn a_standing_hero_holds_frame_zero_when_the_pack_ships_no_idle() {
        // This is what the shipped pack does, and getting it wrong makes the
        // hero flicker on the spot.
        let mut anim = HeroAnimator::new();
        let mut now = 0.0;
        for _ in 0..50 {
            anim.update(now, None, &OPTIONS, false);
            assert_eq!(anim.tick(), 0);
            assert!(!anim.moving());
            now += 0.016;
        }
    }

    #[test]
    fn an_idle_animation_keeps_the_counter_running_while_he_stands() {
        // A pack that ships one wants the hero to breathe, not freeze.
        let mut anim = HeroAnimator::new();
        anim.update(0.0, None, &OPTIONS, true);
        assert_eq!(anim.tick(), 1);
        assert!(!anim.moving(), "breathing is not walking");
        anim.update(INTERVAL, None, &OPTIONS, true);
        assert_eq!(anim.tick(), 2);
    }

    #[test]
    fn the_counter_moves_on_its_own_clock_not_once_per_frame() {
        let mut anim = HeroAnimator::new();
        anim.update(0.0, Some(0.0), &OPTIONS, false);
        assert_eq!(anim.tick(), 1);

        // Sixty frames a second for a fifth of a second: the interval has not
        // elapsed, so the frame does not change.
        let mut now = 0.0;
        for _ in 0..12 {
            now += 0.016;
            anim.update(now, Some(0.0), &OPTIONS, false);
        }
        assert_eq!(anim.tick(), 1, "still inside one interval");

        anim.update(INTERVAL, Some(0.0), &OPTIONS, false);
        assert_eq!(anim.tick(), 2);
    }

    #[test]
    fn the_delay_option_sets_the_interval() {
        // Delay 0 is the fastest the control allows, 0.05s a frame.
        let fast = [0, 1, 1, 1, 1];
        let mut anim = HeroAnimator::new();
        anim.update(0.0, Some(0.0), &fast, false);
        anim.update(0.04, Some(0.0), &fast, false);
        assert_eq!(anim.tick(), 1, "0.04s is short of 0.05s");
        anim.update(0.05, Some(0.0), &fast, false);
        assert_eq!(anim.tick(), 2);

        // Delay 5 is the slowest: 0.30s.
        let slow = [5, 1, 1, 1, 1];
        let mut anim = HeroAnimator::new();
        anim.update(0.0, Some(0.0), &slow, false);
        anim.update(0.29, Some(0.0), &slow, false);
        assert_eq!(anim.tick(), 1);
        anim.update(0.30, Some(0.0), &slow, false);
        assert_eq!(anim.tick(), 2);
    }

    #[test]
    fn walking_stops_once_the_step_is_far_enough_behind() {
        let mut anim = HeroAnimator::new();
        anim.update(0.0, Some(0.0), &OPTIONS, false);
        assert!(anim.moving());

        // Just inside the window.
        anim.update(0.3, Some(0.0), &OPTIONS, false);
        assert!(anim.moving());
        assert_eq!(anim.tick(), 2);

        // Past it: the hero stops and, with no idle set, snaps to frame 0.
        anim.update(0.6, Some(0.0), &OPTIONS, false);
        assert!(!anim.moving());
        assert_eq!(anim.tick(), 0);
    }

    #[test]
    fn turning_animation_off_pins_the_hero_to_frame_zero_mid_walk() {
        let off = [4, 1, 1, 0, 1];
        let mut anim = HeroAnimator::new();
        // Stepping right now, but the option overrules it.
        anim.update(0.0, Some(0.0), &off, false);
        assert!(!anim.moving());
        assert_eq!(anim.tick(), 0);
    }

    #[test]
    fn missing_options_are_treated_as_animation_off() {
        // A pack that declares no options must not index out of range.
        let mut anim = HeroAnimator::new();
        anim.update(0.0, Some(0.0), &[], false);
        assert!(!anim.moving());
        assert_eq!(anim.tick(), 0);
    }
}
