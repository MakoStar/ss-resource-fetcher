use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get     => "GET",
            Self::Post    => "POST",
            Self::Put     => "PUT",
            Self::Patch   => "PATCH",
            Self::Delete  => "DELETE",
            Self::Head    => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<HttpMethod> for reqwest::Method {
    fn from(m: HttpMethod) -> Self {
        match m {
            HttpMethod::Get     => reqwest::Method::GET,
            HttpMethod::Post    => reqwest::Method::POST,
            HttpMethod::Put     => reqwest::Method::PUT,
            HttpMethod::Patch   => reqwest::Method::PATCH,
            HttpMethod::Delete  => reqwest::Method::DELETE,
            HttpMethod::Head    => reqwest::Method::HEAD,
            HttpMethod::Options => reqwest::Method::OPTIONS,
        }
    }
}
