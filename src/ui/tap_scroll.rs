use iced::{
    Element, Event, Length, Point, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::Button,
    touch,
    widget::scrollable,
};

const TAP_SLOP: f32 = 10.0;

#[derive(Debug, Default)]
struct State {
    touch_start: Option<Point>,
    touch_moved: bool,
}

pub struct TapScroll<'a, Message>
where
    Message: 'a,
{
    scrollable: scrollable::Scrollable<'a, Message>,
    on_press: Option<Message>,
}

pub fn tap_scroll<'a, Message>(content: impl Into<Element<'a, Message>>) -> TapScroll<'a, Message>
where
    Message: 'a,
{
    TapScroll {
        scrollable: scrollable(content),
        on_press: None,
    }
}

impl<'a, Message> TapScroll<'a, Message>
where
    Message: 'a,
{
    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn style(
        mut self,
        style: impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'a,
    ) -> Self {
        self.scrollable = self.scrollable.style(style);
        self
    }

    pub fn direction(mut self, direction: scrollable::Direction) -> Self {
        self.scrollable = self.scrollable.direction(direction);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.scrollable = self.scrollable.width(width);
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.scrollable = self.scrollable.height(height);
        self
    }

    pub fn spacing(mut self, spacing: impl Into<iced::Pixels>) -> Self {
        self.scrollable = self.scrollable.spacing(spacing);
        self
    }

    pub fn on_scroll(mut self, on_scroll: impl Fn(scrollable::Viewport) -> Message + 'a) -> Self {
        self.scrollable = self.scrollable.on_scroll(on_scroll);
        self
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for TapScroll<'_, Message>
where
    Message: Clone + 'static,
{
    fn size(&self) -> Size<Length> {
        Widget::size(&self.scrollable)
    }

    fn size_hint(&self) -> Size<Length> {
        Widget::size_hint(&self.scrollable)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        Widget::layout(
            &mut self.scrollable,
            &mut tree.children[0],
            renderer,
            limits,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        Widget::draw(
            &self.scrollable,
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        let child: &dyn Widget<Message, Theme, iced::Renderer> = &self.scrollable;

        vec![Tree::new(child)]
    }

    fn diff(&self, tree: &mut Tree) {
        let child: &dyn Widget<Message, Theme, iced::Renderer> = &self.scrollable;

        tree.diff_children(std::slice::from_ref(&child));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        Widget::operate(
            &mut self.scrollable,
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        Widget::update(
            &mut self.scrollable,
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(Button::Left))
        ) {
            if shell.is_event_captured() {
                return;
            }

            if cursor.is_over(layout.bounds())
                && let Some(message) = &self.on_press
            {
                shell.publish(message.clone());
                shell.capture_event();
            }

            return;
        }

        match event {
            Event::Touch(touch::Event::FingerPressed { position, .. }) => {
                if layout.bounds().contains(*position) {
                    state.touch_start = Some(*position);
                    state.touch_moved = false;
                }
            }

            Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                if let Some(start) = state.touch_start {
                    let dx = position.x - start.x;
                    let dy = position.y - start.y;

                    if dx * dx + dy * dy > TAP_SLOP * TAP_SLOP {
                        state.touch_moved = true;
                    }
                }
            }

            Event::Touch(touch::Event::FingerLifted { .. }) => {
                let should_press = state.touch_start.is_some() && !state.touch_moved;

                state.touch_start = None;
                state.touch_moved = false;

                if should_press && let Some(message) = &self.on_press {
                    shell.publish(message.clone());
                }
            }

            Event::Touch(touch::Event::FingerLost { .. }) => {
                state.touch_start = None;
                state.touch_moved = false;
            }

            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        Widget::mouse_interaction(
            &self.scrollable,
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        Widget::overlay(
            &mut self.scrollable,
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message> From<TapScroll<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a + 'static,
{
    fn from(widget: TapScroll<'a, Message>) -> Self {
        Element::new(widget)
    }
}
