//! "Change times" in the Pomodoro tab: the lengths of focus and breaks, the rounds before the
//! long break, and after how many rounds it stops by itself. Saved in settings.json and used by
//! the next Pomodoro started here; paired devices follow the plan of the one that started it.

use std::rc::Rc;

use slint::SharedString;

use crate::App;
use crate::sync::Plan;

pub fn setup(app: &Rc<App>) {
    app.window.on_edit_plan_clicked(|| crate::with_app(start_editing));
    app.window.on_save_plan(|| crate::with_app(save));
}

fn start_editing(app: &Rc<App>) {
    let plan = app.settings.borrow().pomodoro.sane();
    let window = &app.window;
    window.set_edit_work(plan.work.to_string().into());
    window.set_edit_short_break(plan.short_break.to_string().into());
    window.set_edit_long_break(plan.long_break.to_string().into());
    window.set_edit_rounds(plan.rounds.to_string().into());
    window.set_edit_total(if plan.total == 0 { String::new() } else { plan.total.to_string() }.into());
    window.set_plan_error(SharedString::new());
    window.set_pomodoro_editing(true);
}

fn save(app: &Rc<App>) {
    let window = &app.window;
    let fields = [
        window.get_edit_work(),
        window.get_edit_short_break(),
        window.get_edit_long_break(),
        window.get_edit_rounds(),
        window.get_edit_total(),
    ];
    match read_plan(&fields.each_ref().map(|field| field.as_str())) {
        Ok(plan) => {
            log!("[pomodoro] new plan: {plan:?}");
            let mut settings = app.settings.borrow_mut();
            settings.pomodoro = plan;
            settings.save();
            drop(settings);
            window.set_pomodoro_editing(false);
            crate::update_pomodoro_view(app);
        }
        Err(message) => window.set_plan_error(message.into()),
    }
}

/// Focus, short break, long break, rounds before the long one, rounds in all (empty: no end)
fn read_plan([work, short_break, long_break, rounds, total]: &[&str; 5]) -> Result<Plan, String> {
    let number = |text: &str, name: &str, max: u32| {
        text.trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=max).contains(n))
            .ok_or_else(|| format!("{name}: a number from 1 to {max}"))
    };
    let total = match total.trim() {
        "" | "0" => 0,
        text => number(text, "Stop after", Plan::MAX_TOTAL)?,
    };
    Ok(Plan {
        work: number(work, "Focus", Plan::MAX_WORK)?,
        short_break: number(short_break, "Short break", Plan::MAX_BREAK)?,
        long_break: number(long_break, "Long break", Plan::MAX_BREAK)?,
        rounds: number(rounds, "Long break after", Plan::MAX_ROUNDS)?,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_was_typed() {
        assert_eq!(read_plan(&["25", "5", "15", "4", ""]), Ok(Plan::default()));
        assert_eq!(
            read_plan(&["50", "10", "30", "2", "6"]),
            Ok(Plan { work: 50, short_break: 10, long_break: 30, rounds: 2, total: 6 })
        );
        assert_eq!(read_plan(&["0", "5", "15", "4", ""]), Err("Focus: a number from 1 to 180".into()));
        assert_eq!(read_plan(&["25", "5", "15", "9", ""]), Err("Long break after: a number from 1 to 8".into()));
        assert!(read_plan(&["25", "", "15", "4", ""]).is_err());
    }
}
