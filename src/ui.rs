use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::ai::{AIConfig, AIResponseGenerator};
use crate::api::ApiClient;
use crate::config::Config;
use crate::review::Review;

#[derive(Debug, PartialEq)]
enum AppState {
    ViewingReviews,
    WritingResponse,
    ConfirmingResponse,
    GeneratingAI,
}

#[derive(Debug, PartialEq)]
enum InputMode {
    Manual,
    AI,
}

pub struct ReviewUI {
    api_client: ApiClient,
    ai_generator: Option<AIResponseGenerator>,
    reviews: Vec<Review>,
    selected_review: Option<usize>,
    state: AppState,
    response_text: String,
    cursor_position: usize,
    input_mode: InputMode,
    ai_generated_response: Option<String>,
    loading: bool,
    error_message: Option<String>,
    list_state: ListState,
    config: Config,
}

impl ReviewUI {
    fn get_character_limit(&self) -> Option<usize> {
        match self.config.platform {
            crate::config::Platform::Android => Some(350),
            crate::config::Platform::Ios => None, // No limit for iOS
        }
    }
    
    fn format_text_with_cursor(&self) -> String {
        if self.cursor_position <= self.response_text.len() {
            let mut display_text = self.response_text.clone();
            // Always show static white square cursor
            display_text.insert(self.cursor_position, '█'); // White square cursor
            display_text
        } else {
            self.response_text.clone()
        }
    }

    fn find_next_word_boundary(&self) -> usize {
        let chars: Vec<char> = self.response_text.chars().collect();
        let mut pos = self.cursor_position;
        
        // Skip current word (non-whitespace)
        while pos < chars.len() && !chars[pos].is_whitespace() {
            pos += 1;
        }
        
        // Skip whitespace to next word
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        
        pos
    }

    fn find_prev_word_boundary(&self) -> usize {
        let chars: Vec<char> = self.response_text.chars().collect();
        if self.cursor_position == 0 {
            return 0;
        }
        
        let mut pos = self.cursor_position - 1;
        
        // Skip whitespace backwards
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }
        
        // Skip current word backwards
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        
        pos
    }

    pub async fn new(config: Config) -> Result<Self> {
        let mut api_client = ApiClient::new(config.clone());
        let mut reviews = api_client.get_reviews().await?;

        // Initialize AI generator if OpenAI API key is available
        let ai_generator = if let Some(api_key) = &config.openai_api_key {
            let ai_config = AIConfig {
                openai_api_key: api_key.clone(),
                ..Default::default()
            };
            AIResponseGenerator::new(ai_config).ok()
        } else {
            None
        };

        // Sort reviews by date (newest first)
        reviews.sort_by(|a, b| b.created_date.cmp(&a.created_date));

        let mut list_state = ListState::default();
        if !reviews.is_empty() {
            list_state.select(Some(0));
        }

        let selected_review = if reviews.is_empty() { None } else { Some(0) };

        Ok(Self {
            api_client,
            ai_generator,
            reviews,
            selected_review,
            state: AppState::ViewingReviews,
            response_text: String::new(),
            cursor_position: 0,
            input_mode: InputMode::Manual,
            ai_generated_response: None,
            loading: false,
            error_message: None,
            list_state,
            config,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(250);

        loop {
            terminal.draw(|f| self.ui(f))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match self.handle_input(key).await? {
                        Some(action) => match action {
                            UIAction::Quit => break,
                            UIAction::Refresh => {
                                self.loading = true;
                                match self.api_client.refresh_all_reviews().await {
                                    Ok(mut reviews) => {
                                        // Sort reviews by date (newest first)
                                        reviews.sort_by(|a, b| b.created_date.cmp(&a.created_date));

                                        self.reviews = reviews;
                                        self.selected_review = if self.reviews.is_empty() {
                                            None
                                        } else {
                                            Some(0)
                                        };
                                        if !self.reviews.is_empty() {
                                            self.list_state.select(Some(0));
                                        }
                                        self.error_message = None;
                                    }
                                    Err(e) => {
                                        self.error_message =
                                            Some(format!("Failed to refresh reviews: {}", e));
                                    }
                                }
                                self.loading = false;
                            }
                            UIAction::LoadMore => {
                                self.loading = true;
                                match self.api_client.load_more_reviews().await {
                                    Ok(mut new_reviews) => {
                                        new_reviews
                                            .sort_by(|a, b| b.created_date.cmp(&a.created_date));
                                        self.reviews.extend(new_reviews);
                                        self.error_message = None;
                                    }
                                    Err(e) => {
                                        self.error_message =
                                            Some(format!("Failed to load more reviews: {}", e));
                                    }
                                }
                                self.loading = false;
                            }
                        },
                        None => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    async fn handle_input(&mut self, key: KeyEvent) -> Result<Option<UIAction>> {
        match self.state {
            AppState::ViewingReviews => {
                match key.code {
                    KeyCode::Char('q') => return Ok(Some(UIAction::Quit)),
                    KeyCode::Char('r') => return Ok(Some(UIAction::Refresh)),
                    KeyCode::Char('l') => {
                        if self.api_client.has_more_reviews() {
                            return Ok(Some(UIAction::LoadMore));
                        }
                    }
                    KeyCode::Up => {
                        if let Some(selected) = self.selected_review {
                            if selected > 0 {
                                self.selected_review = Some(selected - 1);
                                self.list_state.select(Some(selected - 1));
                            }
                        }
                    }
                    KeyCode::Down => {
                        if let Some(selected) = self.selected_review {
                            if selected + 1 < self.reviews.len() {
                                self.selected_review = Some(selected + 1);
                                self.list_state.select(Some(selected + 1));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(review_idx) = self.selected_review {
                            // Fetch response data for this review
                            self.loading = true;
                            let review_id = &self.reviews[review_idx].id;
                            match self.api_client.get_review_response(review_id).await {
                                Ok(response) => {
                                    use std::io::Write;
                                    let mut log_file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open("debug.log")
                                        .unwrap_or_else(|_| {
                                            std::fs::File::create("debug.log").unwrap()
                                        });
                                    writeln!(
                                        log_file,
                                        "DEBUG: UI received response: {:?}",
                                        response.is_some()
                                    )
                                    .ok();
                                    if let Some(ref resp) = response {
                                        writeln!(
                                            log_file,
                                            "DEBUG: Response body preview: {}",
                                            &resp.response_body[..resp.response_body.len().min(50)]
                                        )
                                        .ok();
                                    }

                                    self.reviews[review_idx].response = response;
                                    self.state = AppState::WritingResponse;
                                    self.input_mode = InputMode::Manual;
                                    self.response_text.clear();
                                    self.cursor_position = 0;
                                    self.ai_generated_response = None;
                                    self.error_message = None;
                                }
                                Err(e) => {
                                    self.error_message =
                                        Some(format!("Failed to fetch response data: {}", e));
                                }
                            }
                            self.loading = false;
                        }
                    }
                    KeyCode::Char('a') => {
                        if let Some(review_idx) = self.selected_review {
                            // First fetch response data for this review
                            self.loading = true;
                            let review_id = &self.reviews[review_idx].id;
                            match self.api_client.get_review_response(review_id).await {
                                Ok(response) => {
                                    self.reviews[review_idx].response = response;
                                    self.state = AppState::GeneratingAI;
                                    self.input_mode = InputMode::AI;

                                    // Generate AI response (placeholder)
                                    let ai_response = self.generate_ai_response().await?;
                                    self.ai_generated_response = Some(ai_response.clone());
                                    self.response_text = ai_response;
                                    self.cursor_position = self.response_text.len(); // Set cursor at end
                                    self.loading = false;
                                    self.state = AppState::WritingResponse;
                                    self.error_message = None;
                                }
                                Err(e) => {
                                    self.error_message =
                                        Some(format!("Failed to fetch response data: {}", e));
                                    self.loading = false;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppState::WritingResponse => {
                match key.code {
                    KeyCode::Esc => {
                        self.state = AppState::ViewingReviews;
                        self.response_text.clear();
                        self.cursor_position = 0;
                        self.ai_generated_response = None;
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !self.response_text.trim().is_empty() {
                            self.state = AppState::ConfirmingResponse;
                        }
                    }
                    KeyCode::Enter => {
                        // Regular Enter adds a new line at cursor position
                        if let Some(limit) = self.get_character_limit() {
                            if self.response_text.len() < limit {
                                self.response_text.insert(self.cursor_position, '\n');
                                self.cursor_position += 1;
                            }
                        } else {
                            self.response_text.insert(self.cursor_position, '\n');
                            self.cursor_position += 1;
                        }
                    }
                    KeyCode::Char(c) => {
                        // Handle Option+Arrow key sequences that come as characters
                        match c {
                            'b' if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Option+Left (sometimes sent as Alt+b)
                                self.cursor_position = self.find_prev_word_boundary();
                            }
                            'f' if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Option+Right (sometimes sent as Alt+f)
                                self.cursor_position = self.find_next_word_boundary();
                            }
                            'w' if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Option+Backspace: Delete previous word (sometimes sent as Alt+w)
                                let word_start = self.find_prev_word_boundary();
                                if word_start < self.cursor_position {
                                    self.response_text.drain(word_start..self.cursor_position);
                                    self.cursor_position = word_start;
                                }
                            }
                            'w' if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                // Option+Backspace: Delete previous word (sent as Ctrl+w)
                                let word_start = self.find_prev_word_boundary();
                                if word_start < self.cursor_position {
                                    self.response_text.drain(word_start..self.cursor_position);
                                    self.cursor_position = word_start;
                                }
                            }
                            'd' if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Option+d: Delete next word (Alt+d sequence)
                                let word_end = self.find_next_word_boundary();
                                if self.cursor_position < word_end {
                                    self.response_text.drain(self.cursor_position..word_end);
                                }
                            }
                            '\u{0017}' => {
                                // Ctrl+W: Delete previous word (common terminal sequence for Option+Backspace)
                                let word_start = self.find_prev_word_boundary();
                                if word_start < self.cursor_position {
                                    self.response_text.drain(word_start..self.cursor_position);
                                    self.cursor_position = word_start;
                                }
                            }
                            '\u{007f}' if key.modifiers.contains(KeyModifiers::ALT) => {
                                // Option+Backspace: Delete previous word (Alt+DEL sequence)
                                let word_start = self.find_prev_word_boundary();
                                if word_start < self.cursor_position {
                                    self.response_text.drain(word_start..self.cursor_position);
                                    self.cursor_position = word_start;
                                }
                            }
                            _ => {
                                // Check character limit before inserting
                                if let Some(limit) = self.get_character_limit() {
                                    if self.response_text.len() < limit {
                                        self.response_text.insert(self.cursor_position, c);
                                        self.cursor_position += 1;
                                    }
                                } else {
                                    self.response_text.insert(self.cursor_position, c);
                                    self.cursor_position += 1;
                                }
                            }
                        }
                    }
                    KeyCode::Left => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // Option+Left: Jump to previous word
                            self.cursor_position = self.find_prev_word_boundary();
                        } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Cmd+Left: Jump to beginning of line (treat as Home)
                            self.cursor_position = 0;
                        } else if self.cursor_position > 0 {
                            self.cursor_position -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // Option+Right: Jump to next word
                            self.cursor_position = self.find_next_word_boundary();
                        } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Cmd+Right: Jump to end of line (treat as End)
                            self.cursor_position = self.response_text.len();
                        } else if self.cursor_position < self.response_text.len() {
                            self.cursor_position += 1;
                        }
                    }
                    KeyCode::Home => {
                        self.cursor_position = 0;
                    }
                    KeyCode::End => {
                        self.cursor_position = self.response_text.len();
                    }
                    KeyCode::Backspace => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // Option+Backspace: Delete previous word
                            let word_start = self.find_prev_word_boundary();
                            if word_start < self.cursor_position {
                                self.response_text.drain(word_start..self.cursor_position);
                                self.cursor_position = word_start;
                            }
                        } else if self.cursor_position > 0 {
                            self.cursor_position -= 1;
                            self.response_text.remove(self.cursor_position);
                        }
                    }
                    KeyCode::Delete => {
                        if self.cursor_position < self.response_text.len() {
                            self.response_text.remove(self.cursor_position);
                        }
                    }
                    _ => {}
                }
            }
            AppState::ConfirmingResponse => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(review_idx) = self.selected_review {
                        let review_id = &self.reviews[review_idx].id;
                        match self
                            .api_client
                            .submit_response(review_id, &self.response_text)
                            .await
                        {
                            Ok(()) => {
                                self.error_message =
                                    Some("Response submitted successfully!".to_string());
                            }
                            Err(e) => {
                                self.error_message =
                                    Some(format!("Failed to submit response: {}", e));
                            }
                        }
                    }
                    self.state = AppState::ViewingReviews;
                    self.response_text.clear();
                    self.ai_generated_response = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.state = AppState::WritingResponse;
                }
                _ => {}
            },
            AppState::GeneratingAI => {
                // Do nothing while generating
            }
        }

        Ok(None)
    }

    async fn generate_ai_response(&self) -> Result<String> {
        if let Some(ai_generator) = &self.ai_generator {
            if let Some(review_idx) = self.selected_review {
                let review = &self.reviews[review_idx];
                ai_generator.generate_response(review).await
            } else {
                Ok("Thank you for your feedback!".to_string())
            }
        } else {
            // Fallback to simple response if no AI available
            if let Some(review_idx) = self.selected_review {
                let review = &self.reviews[review_idx];
                let response = format!(
                    "Thank you for your {}-star review{}! We appreciate your feedback and are constantly working to improve our app.",
                    review.rating,
                    if let Some(title) = &review.title {
                        format!(" about \"{}\"", title)
                    } else {
                        String::new()
                    }
                );
                Ok(response)
            } else {
                Ok("Thank you for your feedback!".to_string())
            }
        }
    }

    fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        let size = f.size();

        match self.state {
            AppState::ViewingReviews => self.draw_reviews_view(f, size),
            AppState::WritingResponse => self.draw_response_view(f, size),
            AppState::ConfirmingResponse => self.draw_confirmation_view(f, size),
            AppState::GeneratingAI => self.draw_loading_view(f, size),
        }

        // Draw error message if present
        if let Some(error) = &self.error_message {
            let popup_area = centered_rect(60, 20, size);
            f.render_widget(Clear, popup_area);
            let error_paragraph = Paragraph::new(error.as_ref())
                .block(Block::default().borders(Borders::ALL).title("Message"))
                .wrap(Wrap { trim: true });
            f.render_widget(error_paragraph, popup_area);

            // Clear error after showing
            self.error_message = None;
        }
    }

    fn draw_reviews_view<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        // Create a layout that properly separates content from help
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),   // Main content area (reviews)
                Constraint::Length(8), // Help section (fixed height)
            ])
            .split(area);

        // Split the main content area for reviews
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_chunks[0]);

        // Reviews list
        let reviews: Vec<ListItem> = self
            .reviews
            .iter()
            .enumerate()
            .map(|(_i, review)| {
                let rating_stars = "⭐".repeat(review.rating as usize);
                let content = format!(
                    "{} {} - {}",
                    rating_stars,
                    review.reviewer_nickname,
                    review.created_date.format("%Y-%m-%d")
                );
                ListItem::new(content)
            })
            .collect();

        let reviews_list = List::new(reviews)
            .block(Block::default().borders(Borders::ALL).title("Reviews"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        f.render_stateful_widget(reviews_list, content_chunks[0], &mut self.list_state);

        // Review details
        if let Some(review_idx) = self.selected_review {
            let review = &self.reviews[review_idx];
            let rating_stars = "⭐".repeat(review.rating as usize);

            let mut text = vec![
                Spans::from(vec![Span::styled(
                    format!("Rating: {}", rating_stars),
                    Style::default().fg(Color::Yellow),
                )]),
                Spans::from(vec![Span::raw(format!(
                    "Reviewer: {}",
                    review.reviewer_nickname
                ))]),
                Spans::from(vec![Span::raw(format!(
                    "Date: {}",
                    review.created_date.format("%Y-%m-%d %H:%M")
                ))]),
                Spans::from(vec![Span::raw(format!("Territory: {}", review.territory))]),
            ];

            // Add version info if available
            if let Some(version) = &review.version {
                text.push(Spans::from(vec![Span::raw(format!(
                    "Version: {}",
                    version
                ))]));
            }

            text.push(Spans::from(vec![Span::raw("")]));

            if let Some(title) = &review.title {
                text.push(Spans::from(vec![Span::styled(
                    format!("Title: {}", title),
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
            }

            if let Some(body) = &review.body {
                text.push(Spans::from(vec![Span::raw("")]));
                text.push(Spans::from(vec![Span::styled(
                    "Review:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
                text.push(Spans::from(vec![Span::raw(body)]));
            }

            // Show existing response if available
            if let Some(response) = &review.response {
                text.push(Spans::from(vec![Span::raw("")]));
                text.push(Spans::from(vec![Span::styled(
                    "✅ Developer Response:",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                text.push(Spans::from(vec![Span::styled(
                    &response.response_body,
                    Style::default().fg(Color::Green),
                )]));
                text.push(Spans::from(vec![Span::styled(
                    format!(
                        "Responded: {}",
                        response.last_modified_date.format("%Y-%m-%d %H:%M")
                    ),
                    Style::default().fg(Color::Gray),
                )]));
            } else {
                text.push(Spans::from(vec![Span::raw("")]));
                text.push(Spans::from(vec![Span::styled(
                    "Press Enter to respond or 'a' for AI response",
                    Style::default().fg(Color::Yellow),
                )]));
            }

            let review_detail = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Review Details"),
                )
                .wrap(Wrap { trim: true });

            f.render_widget(review_detail, content_chunks[1]);
        }

        // Instructions in separate area (opaque background)
        let help_text = vec![
            Spans::from("Controls:"),
            Spans::from("↑/↓ - Navigate reviews"),
            Spans::from("Enter - Write manual response"),
            Spans::from("'a' - Generate AI response"),
            Spans::from("'r' - Refresh reviews"),
            Spans::from("'l' - Load more reviews (Android)"),
            Spans::from("'q' - Quit"),
        ];

        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .style(Style::default().fg(Color::Gray).bg(Color::Black))
            .wrap(Wrap { trim: true });

        f.render_widget(help_paragraph, main_chunks[1]);
    }

    fn draw_response_view<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(8), // Original review
                    Constraint::Length(6), // Existing response (if any)
                    Constraint::Min(8),    // Your response input
                ]
                .as_ref(),
            )
            .split(area);

        // Show current review at the top
        if let Some(review_idx) = self.selected_review {
            let review = &self.reviews[review_idx];
            let rating_stars = "⭐".repeat(review.rating as usize);

            let review_text = vec![
                Spans::from(vec![Span::styled(
                    format!(
                        "Responding to: {} - {}",
                        rating_stars, review.reviewer_nickname
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Spans::from(vec![Span::raw("")]),
                Spans::from(vec![Span::styled(
                    review.title.as_deref().unwrap_or("(No title)"),
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Spans::from(vec![Span::raw(
                    review.body.as_deref().unwrap_or("(No review text)"),
                )]),
            ];

            let review_paragraph = Paragraph::new(review_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Original Review"),
                )
                .wrap(Wrap { trim: true });

            f.render_widget(review_paragraph, chunks[0]);

            // Show existing developer response if it exists
            if let Some(response) = &review.response {
                let response_text = vec![
                    Spans::from(vec![Span::styled(
                        "⚠️  ALREADY RESPONDED:",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )]),
                    Spans::from(vec![Span::styled(
                        &response.response_body,
                        Style::default().fg(Color::Yellow),
                    )]),
                    Spans::from(vec![Span::styled(
                        format!(
                            "Sent: {}",
                            response.last_modified_date.format("%Y-%m-%d %H:%M")
                        ),
                        Style::default().fg(Color::Gray),
                    )]),
                ];

                let response_paragraph = Paragraph::new(response_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Existing Developer Response"),
                    )
                    .wrap(Wrap { trim: true });

                f.render_widget(response_paragraph, chunks[1]);

                // Response input (smaller since existing response is shown)
                let input_title = if let Some(limit) = self.get_character_limit() {
                    format!("⚠️  Update/Replace Response ({}/{} chars - Ctrl+S to submit, Esc to cancel)", 
                           self.response_text.len(), limit)
                } else {
                    "⚠️  Update/Replace Response (Ctrl+S to submit, Esc to cancel)".to_string()
                };
                let display_text = self.format_text_with_cursor();
                let response_input = Paragraph::new(display_text.as_ref())
                    .block(Block::default().borders(Borders::ALL).title(input_title))
                    .wrap(Wrap { trim: true });

                f.render_widget(response_input, chunks[2]);
            } else {
                // No existing response - show larger input area
                let empty_text = vec![Spans::from(vec![Span::styled(
                    "✅ No existing response - you can write a new one",
                    Style::default().fg(Color::Green),
                )])];

                let no_response_paragraph = Paragraph::new(empty_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Response Status"),
                    )
                    .wrap(Wrap { trim: true });

                f.render_widget(no_response_paragraph, chunks[1]);

                let input_title = match self.input_mode {
                    InputMode::Manual => {
                        if let Some(limit) = self.get_character_limit() {
                            format!("Write Response ({}/{} chars - Ctrl+S to submit, Esc to cancel)", 
                                   self.response_text.len(), limit)
                        } else {
                            "Write Response (Ctrl+S to submit, Esc to cancel)".to_string()
                        }
                    },
                    InputMode::AI => {
                        if let Some(limit) = self.get_character_limit() {
                            format!("AI Generated Response ({}/{} chars - Edit if needed, Ctrl+S to submit, Esc to cancel)", 
                                   self.response_text.len(), limit)
                        } else {
                            "AI Generated Response (Edit if needed, Ctrl+S to submit, Esc to cancel)".to_string()
                        }
                    }
                };

                let display_text = self.format_text_with_cursor();
                let response_input = Paragraph::new(display_text.as_ref())
                    .block(Block::default().borders(Borders::ALL).title(input_title))
                    .wrap(Wrap { trim: true });

                f.render_widget(response_input, chunks[2]);
            }
        }
    }

    fn draw_confirmation_view<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        let popup_area = centered_rect(80, 60, area);
        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(popup_area);

        // Confirmation prompt
        let confirmation = Paragraph::new("Submit this response? (y/n)")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Confirm Response"),
            )
            .style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(confirmation, chunks[0]);

        // Response preview
        let response_preview = Paragraph::new(self.response_text.as_ref())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Response Preview"),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(response_preview, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("Press 'y' to submit, 'n' or Esc to go back")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

        f.render_widget(instructions, chunks[2]);
    }

    fn draw_loading_view<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        let popup_area = centered_rect(40, 20, area);
        f.render_widget(Clear, popup_area);

        let loading_text = Paragraph::new("Generating AI response...")
            .block(Block::default().borders(Borders::ALL).title("Please Wait"))
            .style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(loading_text, popup_area);
    }
}

enum UIAction {
    Quit,
    Refresh,
    LoadMore,
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
