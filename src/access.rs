// Bugzilla API documentation:
// https://bugzilla.redhat.com/docs/en/html/api/core/v1/general.html

use restson::{Error, Response as RestResponse, RestClient, RestPath};
use restson::blocking::RestClient as BlockingRestClient;

use crate::bug_model::{Bug, Response};

/// Configuration and credentials to access a Bugzilla instance.
pub struct BzInstance {
    pub host: String,
    pub auth: Auth,
    pub pagination: Pagination,
    pub included_fields: Vec<String>,
    client: BlockingRestClient,
}

/// The authentication method that the crate uses when contacting Bugzilla.
#[derive(Default)]
pub enum Auth {
    #[default]
    Anonymous,
    ApiKey(String),
}

/// Controls the upper limit of how many bugs the response from Bugzilla can contain:
///
/// * `Default`: Use the default settings of this instance, which sets an arbitrary limit on the number of bugs.
/// * `Limit`: Use this upper limit instead.
/// * `Unlimited`: Set the limit to 0, which disables the upper limit and returns all matching bugs.
#[derive(Default)]
pub enum Pagination {
    #[default]
    Default,
    Limit(u32),
    Unlimited,
}

impl Pagination {
    /// Format the `Pagination` variant as a URL query fragment, such as `?limit=20`.
    fn as_query(&self) -> String {
        match self {
            Pagination::Default => String::new(),
            Pagination::Limit(n) => format!("&limit={}", n),
            Pagination::Unlimited => "&limit=0".to_string(),
        }
    }
}

/// This struct temporarily groups together all the parameters to make a REST request.
/// It exists here because `RestPath` is only generic over a single parameter.
struct Request<'a> {
    ids: &'a [&'a str],
    pagination: &'a Pagination,
    fields: &'a str,
}

// TODO: Make this generic over &[&str] and &[String].
/// API call with several &str parameter, which are the bug IDs.
impl RestPath<Request<'_>> for Response {
    fn get_path(request: Request) -> Result<String, Error> {
        Ok(format!(
            "rest/bug?id={}{}{}",
            request.ids.join(","),
            request.fields,
            request.pagination.as_query()
        ))
    }
}

impl BzInstance {
    /// Create a new `BzInstance` struct using a host URL, with default values
    /// for all options.
    pub fn at(host: String) -> Result<Self, Error> {
        // TODO: This function takes host as a String, even though client is happy with &str.
        // The String is only used in the host struct attribute.
        let client = RestClient::builder().blocking(&host)?;

        Ok(BzInstance {
            host,
            client,
            included_fields: vec!["_default".to_string()],
            auth: Auth::default(),
            pagination: Pagination::default(),
        })
    }

    /// Set the authentication method of this `BzInstance`.
    pub fn authenticate(mut self, auth: Auth) -> Result<Self, Error> {
        self.auth = auth;
        // If the user selects the API key authorization, set the API key in the request header.
        // Otherwise, the anonymous authorization doesn't modify the request in any way.
        if let Auth::ApiKey(key) = &self.auth {
            self.client.set_header("Authorization", &format!("Bearer {}", key))?;
        }
        Ok(self)
    }

    /// Set the pagination method of this `BzInstance`.
    #[must_use]
    pub fn paginate(mut self, pagination: Pagination) -> Self {
        self.pagination = pagination;
        self
    }

    /// Set Bugzilla fields that this `BzInstance` will request, such as `flags`.
    ///
    /// By default, `BzInstance` requests the `_default` fields, and using this method
    /// overwrites the default value. If you want to set fields in addition
    /// to `_default`, specify `_default` in your list.
    #[must_use]
    pub fn include_fields(mut self, fields: Vec<String>) -> Self {
        self.included_fields = fields;
        self
    }

    /// Format the included Bugzilla fields as a URL query fragment, such as `&include_fields=_default,flags`.
    #[must_use]
    fn fields_as_query(&self) -> String {
        if self.included_fields.is_empty() {
            String::new()
        } else {
            format!("&include_fields={}", self.included_fields.join(","))
        }
    }

    /// Access several bugs by their IDs.
    pub fn bugs(&self, ids: &[&str]) -> Result<Vec<Bug>, Error> {
        let request = Request {
            ids,
            pagination: &self.pagination,
            fields: &self.fields_as_query(),
        };

        // Gets a bug by ID and deserializes the JSON to data variable
        let data: RestResponse<Response> = self.client.get(request)?;
        let response = data.into_inner();
        log::debug!("{:#?}", response);

        // TODO: Note that the resulting list might be empty and still Ok
        Ok(response.bugs)
    }

    /// Access a single bug by its ID.
    pub fn bug(&self, id: &str) -> Result<Bug, Error> {
        // Reuse the `bugs` function. Later, extract the first element.
        let bugs = self.bugs(&[id])?;

        // This is a way to return the first (and only) element of the Vec,
        // without cloning it.
        // TODO: I'm using InvalidValue here mostly as a placeholder.
        // The response should always contain one bug, but if it doesn't,
        // I don't know how best to report it. Maybe just panic?
        bugs.into_iter().next().ok_or(Error::InvalidValue)
    }
}
