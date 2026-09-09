use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::{error, warn};

const IGNORED: [&str; 0] = [];
const WARNED: [&str; 1] = ["Webhook retry cooldown not yet passed"];

#[derive(Debug, Clone)]
pub struct Location {
    file: String,
    line: u32,
    column: u32,
}

impl Location {
    pub fn new(location: std::panic::Location<'static>) -> Self {
        let file = location.file();
        Self {
            file: file[4..file.len() - 3].to_string(),
            line: location.line(),
            column: location.column(),
        }
    }

    #[allow(dead_code)] // false positive
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(self.file.as_bytes());
        output.extend_from_slice(self.line.to_string().as_bytes());
        output.push(b':');
        output.extend_from_slice(self.column.to_string().as_bytes());
        output
    }

    pub fn _from_bytes(bytes: &[u8]) -> Result<Vec<Self>> {
        let string = std::str::from_utf8(bytes)?;
        let mut locations = Vec::new();
        for location in string.split('>') {
            let mut file = String::new();
            let mut line = String::new();
            let mut column = String::new();
            for (i, c) in location.chars().enumerate() {
                if c == ':' {
                    column = location[i + 1..].to_string();
                    break;
                }
                if c.is_ascii_digit() {
                    line.push(c)
                } else {
                    file.push(c)
                }
            }
            locations.push(Self {
                file,
                line: line.parse()?,
                column: column.parse()?,
            });
        }
        Ok(locations)
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
    pub location: Vec<Location>,
}

impl Error {
    #[track_caller]
    pub fn new<T: std::fmt::Display>(message: T) -> Self {
        let location = Location::new(*std::panic::Location::caller());
        let message = message.to_string();

        let new = Self {
            message: message.clone(),
            location: vec![location.clone()],
        };

        for i in WARNED {
            let regex = match regex::Regex::new(i) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to compile regex '{}': {}", i, e);
                    continue;
                }
            };
            if regex.is_match(&message) {
                warn!("{} {}", location, message);
                return new;
            }
        }

        for i in IGNORED {
            let regex = match regex::Regex::new(i) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to compile regex '{}': {}", i, e);
                    continue;
                }
            };
            if regex.is_match(&message) {
                return new;
            }
        }
        error!("{} {}", location, message);
        new
    }

    pub fn into_http_error(self, status: StatusCode) -> HttpError {
        HttpError {
            status,
            error: self,
        }
    }
}

pub trait AddLocation {
    fn add(self) -> Self;
}

pub type Result<T> = std::result::Result<T, Error>;

impl<T> AddLocation for Result<T> {
    #[track_caller]
    fn add(self) -> Self {
        match self {
            Ok(o) => Ok(o),
            Err(e) => {
                let mut location = e.location.clone();
                location.push(Location::new(*std::panic::Location::caller()));
                let bytes = &mut location
                    .iter()
                    .flat_map(|l| {
                        let mut b = l.as_bytes();
                        b.push(b'>');
                        b
                    })
                    .collect::<Vec<u8>>();
                bytes.pop();

                let new = Err(Error {
                    message: e.message.clone(),
                    location: location.clone(),
                });
                for i in WARNED {
                    let regex = match regex::Regex::new(i) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("Failed to compile regex '{}': {}", i, e);
                            continue;
                        }
                    };
                    if regex.is_match(&e.message) {
                        warn!(
                            "{} {}",
                            location
                                .iter()
                                .map(|l| l.to_string())
                                .collect::<Vec<String>>()
                                .join(">"),
                            e.message
                        );
                        return new;
                    }
                }
                for i in IGNORED {
                    let regex = match regex::Regex::new(i) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("Failed to compile regex '{}': {}", i, e);
                            continue;
                        }
                    };
                    if regex.is_match(&e.message) {
                        return new;
                    }
                }

                error!(
                    "{} {}",
                    location
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<String>>()
                        .join(">"),
                    e.message
                );
                new
            }
        }
    }
}

impl<T: std::error::Error> From<T> for Error {
    #[track_caller]
    fn from(message: T) -> Error {
        let location = Location::new(*std::panic::Location::caller());
        let message = message.to_string();

        let new = Self {
            message: message.clone(),
            location: vec![location.clone()],
        };

        for i in WARNED {
            let regex = match regex::Regex::new(i) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to compile regex '{}': {}", i, e);
                    continue;
                }
            };
            if regex.is_match(message.as_str()) {
                warn!("{} {}", location, message);
                return new;
            }
        }
        for i in IGNORED {
            let regex = match regex::Regex::new(i) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to compile regex '{}': {}", i, e);
                    continue;
                }
            };
            if regex.is_match(message.as_str()) {
                return new;
            }
        }
        error!("{} {}", location, message);
        new
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let location = self
            .location
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<String>>()
            .join(">");
        write!(f, "{} {}", location, self.message)
    }
}

pub type HttpResult<T> = std::result::Result<T, HttpError>;

pub struct HttpError {
    pub status: StatusCode,
    pub error: Error,
}

impl<T: std::error::Error> From<T> for HttpError {
    fn from(error: T) -> Self {
        let error = Error::from(error);
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error,
        }
    }
}

impl Into<HttpError> for Error {
    fn into(self) -> HttpError {
        HttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: self,
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let body = format!("Error: {}\n", self.error);
        (self.status, body).into_response()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let body = format!("Error: {}\n", self);
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
