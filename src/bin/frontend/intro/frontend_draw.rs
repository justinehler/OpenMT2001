use crate::live_host::ResultReview;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Layer {
    Background,
    Menu,
    Statistics,
}

pub(super) fn layers(review: Option<ResultReview>) -> &'static [Layer] {
    // Game.End has torn down the city. The stopped graph (49dcb0) owns the
    // frontend alone; the form that launched the game must not show through.
    match review {
        Some(ResultReview::Statistics) => &[Layer::Background, Layer::Statistics],
        _ => &[Layer::Background, Layer::Menu],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_host::ResultMenu;

    #[test]
    fn result_review_draws_background_then_statistics_until_ok_returns_to_menu() {
        assert_eq!(layers(None), &[Layer::Background, Layer::Menu]);
        assert_eq!(
            layers(Some(ResultReview::Statistics)),
            &[Layer::Background, Layer::Statistics],
        );
        for target in [ResultMenu::Scenarios, ResultMenu::Main] {
            assert_eq!(
                layers(Some(ResultReview::Menu(target))),
                &[Layer::Background, Layer::Menu],
            );
        }
    }
}
