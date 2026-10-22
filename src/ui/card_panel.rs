use crate::ui::calendar::{Calendar, CalendarMessage};
use crate::ui::tap_scroll::tap_scroll;
use crate::ui::theme::{line_height, style, text_size};
use iced::{
    Color, Element, Length, Subscription,
    widget::{Space, column, container, row, scrollable, text},
};
use serde::{Deserialize, Serialize};
use crate::backend::app_state::global_app_state;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Card {
    Calendar,
    Led,
}

#[derive(Debug, Clone)]
pub enum CardPanelMessage {
    Calendar(CalendarMessage),
    Next,
    Previous,
    Pressed,
}

pub struct CardPanel {
    calendar: Option<Calendar>,
    cards: Vec<Card>,
    active: usize,
}

impl CardPanel {
    pub fn new() -> Self {
        let cards = global_app_state().config.blocking_read().cards.clone();

        let calendar = if cards.contains(&Card::Calendar) {
            Some(Calendar::new())
        } else {
            None
        };

        Self {
            calendar,
            cards,
            active: 0,
        }
    }

    pub fn update(&mut self, message: CardPanelMessage) {
        match message {
            CardPanelMessage::Calendar(message) => {
                if let Some(calendar) = &mut self.calendar {
                    calendar.update(message);
                }
            }
            CardPanelMessage::Next => {
                self.next();
            }
            CardPanelMessage::Previous => {
                self.prev();
            }
            // the parent View handles this message.
            CardPanelMessage::Pressed => {}
        }
    }

    pub fn subscription(&self) -> Subscription<CardPanelMessage> {
        if let Some(calendar) = &self.calendar {
            calendar.subscription().map(CardPanelMessage::Calendar)
        } else {
            Subscription::none()
        }
    }

    pub fn view(&self) -> Element<'_, CardPanelMessage> {
        if self.cards.is_empty() {
            return text("No cards configured")
                .size(text_size())
                .line_height(line_height())
                .into();
        }

        let content: Element<'_, CardPanelMessage> = match self.cards[self.active] {
            Card::Calendar => {
              if let Some(calendar) = &self.calendar {
                  calendar.view().map(CardPanelMessage::Calendar)
              } else {
                    text("Calendar card disabled")
                        .size(text_size())
                        .line_height(line_height())
                        .into()
              }
            },
            // TODO: implement LED card.
            Card::Led => text("LED (placeholder)")
                .size(text_size())
                .line_height(line_height())
                .into(),
        };

        let scroll = tap_scroll(content)
            .style(|theme, status| {
                let mut s = scrollable::default(theme, status);
                s.vertical_rail.scroller.background = style().text_color.into();
                s.horizontal_rail.scroller.background = style().text_color.into();
                s
            })
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(2).scroller_width(2),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(CardPanelMessage::Pressed)
            .on_swipe_left(CardPanelMessage::Next)
            .on_swipe_right(CardPanelMessage::Previous);

        column![scroll, self.position_indicator(),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn active_index(&self) -> usize {
        self.cards
            .iter()
            .position(|card| *card == self.cards[self.active])
            .expect("active card must be present in Card::ALL")
    }

    fn position_indicator(&self) -> Element<'_, CardPanelMessage> {
        if self.cards.len() <= 1 {
            return Space::new().into();
        }

        let active_index = self.active_index();

        let segments: Vec<Element<'_, CardPanelMessage>> = self.cards
            .iter()
            .enumerate()
            .map(|(index, _)| {
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fixed(2.0))
                    .style(move |_| {
                        let color = style().text_color;

                        let color = if index == active_index {
                            color
                        } else {
                            Color {
                                a: color.a * 0.5,
                                ..color
                            }
                        };

                        container::Style {
                            background: Some(color.into()),
                            ..Default::default()
                        }
                    })
                    .into()
            })
            .collect();

        container(row(segments).spacing(5).width(Length::Fill))
            .width(Length::Fill)
            .padding([0, 2])
            .into()
    }

    pub fn next(&mut self) -> bool {
        let current_index = self.active_index();
        let next_index = (current_index + 1) % self.cards.len();
        let wrapped = next_index == 0;

        self.active = next_index;

        wrapped
    }

    pub fn prev(&mut self) -> bool {
        let current_index = self.active_index();
        let wrapped = current_index == 0;

        let previous_index = if wrapped {
            self.cards.len() - 1
        } else {
            current_index - 1
        };

        self.active = previous_index;

        wrapped
    }
}
