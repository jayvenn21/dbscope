//! Terminal styling for dbscope CLI output.
//! Uses ANSI escape codes with automatic detection of TTY support.

use std::io::IsTerminal;

pub struct Theme {
    pub enabled: bool,
}

impl Theme {
    pub fn detect() -> Self {
        let enabled = std::io::stderr().is_terminal()
            && std::env::var("NO_COLOR").is_err()
            && std::env::var("TERM").map_or(true, |t| t != "dumb");
        Self { enabled }
    }

    pub fn brand(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;79;70;229m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn bold(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[1m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn dim(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[2m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn risk_critical(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;220;38;38m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn risk_high(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;234;88;12m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn risk_medium(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;217;119;6m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn risk_low(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;5;150;105m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn heading(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[1;38;2;79;70;229m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn muted(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[38;2;148;163;184m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn value(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[1;38;2;15;23;42m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn risk_color(&self, score: f64, s: &str) -> String {
        if score >= 0.75 {
            self.risk_critical(s)
        } else if score >= 0.5 {
            self.risk_high(s)
        } else if score >= 0.25 {
            self.risk_medium(s)
        } else {
            self.risk_low(s)
        }
    }
}
