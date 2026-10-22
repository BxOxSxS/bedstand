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
const SWIPE_THRESHOLD: f32 = 40.0;
const SWIPE_AXIS_RATIO: f32 = 1.2;

pub struct TapScroll<'a, Message>
where
    Message: 'a,
{
    scrollable: scrollable::Scrollable<'a, Message>,
    on_press: Option<Message>,
    on_swipe_left: Option<Message>,
    on_swipe_right: Option<Message>,
}

pub fn tap_scroll<'a, Message>(content: impl Into<Element<'a, Message>>) -> TapScroll<'a, Message>
where
    Message: 'a,
{
    TapScroll {
        scrollable: scrollable(content),
        on_press: None,
        on_swipe_left: None,
        on_swipe_right: None,
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

    pub fn on_swipe_left(mut self, message: Message) -> Self {
        self.on_swipe_left = Some(message);
        self
    }

    pub fn on_swipe_right(mut self, message: Message) -> Self {
        self.on_swipe_right = Some(message);
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
        let mut mouse_press_started = false;
        let mut mouse_gesture = None;
        let mut touch_gesture = None;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(Button::Left)) => {
                let start = cursor
                    .position()
                    .filter(|position| layout.bounds().contains(*position));
                mouse_press_started = start.is_some();

                if let Some(position) = start {
                    state.mouse.begin(position);
                } else {
                    state.mouse.cancel();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                state.mouse.update(*position);
            }
            Event::Mouse(mouse::Event::ButtonReleased(Button::Left)) => {
                mouse_gesture = state.mouse.finish(cursor.position());
            }
            Event::Touch(touch::Event::FingerPressed { position, .. }) => {
                if layout.bounds().contains(*position) {
                    state.touch.begin(*position);
                }
            }
            Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                state.touch.update(*position);
            }
            Event::Touch(touch::Event::FingerLifted { position, .. }) => {
                touch_gesture = state.touch.finish(Some(*position));
            }
            Event::Touch(touch::Event::FingerLost { .. }) => {
                state.touch.cancel();
            }
            _ => {}
        }

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

        if mouse_press_started && shell.is_event_captured() {
            state.mouse.cancel();
        }

        if let Some(gesture) = mouse_gesture {
            publish_gesture(
                gesture,
                &self.on_press,
                &self.on_swipe_left,
                &self.on_swipe_right,
                shell,
                true,
            );
        }

        if let Some(gesture) = touch_gesture {
            publish_gesture(
                gesture,
                &self.on_press,
                &self.on_swipe_left,
                &self.on_swipe_right,
                shell,
                !matches!(gesture, Gesture::Tap),
            );
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

#[derive(Debug, Default)]
struct State {
    touch: GestureTrack,
    mouse: GestureTrack,
}

#[derive(Debug, Default)]
struct GestureTrack {
    start: Option<Point>,
    last: Option<Point>,
    moved_beyond_tap_slop: bool,
}

impl GestureTrack {
    fn begin(&mut self, position: Point) {
        self.start = Some(position);
        self.last = Some(position);
        self.moved_beyond_tap_slop = false;
    }

    fn update(&mut self, position: Point) {
        let Some(start) = self.start else {
            return;
        };

        let dx = position.x - start.x;
        let dy = position.y - start.y;

        if dx * dx + dy * dy > TAP_SLOP * TAP_SLOP {
            self.moved_beyond_tap_slop = true;
        }

        self.last = Some(position);
    }

    fn finish(&mut self, end: Option<Point>) -> Option<Gesture> {
        let Some(start) = self.start else {
            self.cancel();
            return None;
        };

        let end = end.or(self.last).unwrap_or(start);
        let moved = self.moved_beyond_tap_slop;

        self.cancel();

        classify_gesture(start, end, moved)
    }

    fn cancel(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesture {
    Tap,
    SwipeLeft,
    SwipeRight,
}

fn classify_gesture(start: Point, end: Point, moved_beyond_tap_slop: bool) -> Option<Gesture> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;

    if dx.abs() >= SWIPE_THRESHOLD && dx.abs() > dy.abs() * SWIPE_AXIS_RATIO {
        return Some(if dx < 0.0 {
            Gesture::SwipeLeft
        } else {
            Gesture::SwipeRight
        });
    }

    if !moved_beyond_tap_slop && dx * dx + dy * dy <= TAP_SLOP * TAP_SLOP {
        Some(Gesture::Tap)
    } else {
        None
    }
}

fn publish_gesture<Message: Clone + 'static>(
    gesture: Gesture,
    on_press: &Option<Message>,
    on_swipe_left: &Option<Message>,
    on_swipe_right: &Option<Message>,
    shell: &mut Shell<'_, Message>,
    capture: bool,
) {
    let message = match gesture {
        Gesture::Tap => on_press,
        Gesture::SwipeLeft => on_swipe_left,
        Gesture::SwipeRight => on_swipe_right,
    };

    if let Some(message) = message {
        shell.publish(message.clone());
        if capture {
            shell.capture_event();
        }
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
