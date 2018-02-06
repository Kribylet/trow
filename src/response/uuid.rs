use failure::Error;
use rocket::State;
use rocket::http::{Header, Status};
use rocket::response::{Responder, Response};
use rocket::request::Request;
use hostname;
use uuid::Uuid;
use response::errors;

use grpc::backend;
use config;
use types::Layer;

#[derive(Debug, Serialize)]
pub enum UuidResponse {
    Uuid {
        uuid: String,
        name: String,
        repo: String,
        range: (u32, u32),
    },
    Empty,
}

impl UuidResponse {
    pub fn handle(
        handler: State<config::BackendHandler>,
        name: String,
        repo: String,
    ) -> Result<UuidResponse, Error> {
        let backend = handler.backend();
        let mut req = backend::Layer::new();
        req.set_name(name.clone());
        req.set_repo(repo.clone());

        let response = backend.gen_uuid(req)?;
        debug!("Client received: {:?}", response);

        Ok(UuidResponse::Uuid {
            uuid: response.get_uuid().to_owned(),
            name: name,
            repo: repo,
            range: (0, 0)
        })
    }

    pub fn uuid_exists(
        handler: State<config::BackendHandler>,
        layer: &Layer,
    ) -> Result<bool, Error> {
        let backend = handler.backend();
        let mut req = backend::Layer::new();
        req.set_name(layer.name.to_owned());
        req.set_repo(layer.repo.to_owned());
        req.set_digest(layer.digest.to_owned());

        let response = backend.uuid_exists(req)?;
        debug!("UuidExists: {:?}", response.get_success());
        match response.get_success() {
            true => Ok(true),
            false => Err(errors::Error::DigestInvalid.into()),
        }
    }
}

fn _gen_uuid() -> Uuid {
    Uuid::new_v4()
}

/// Gets the base URL e.g. http://registry:8000 using the HOST value from the request header.
/// Falls back to hostname if it doesn't exist.
///
fn get_base_url(req: &Request) -> String {
    let host = match req.headers().get("HOST").next() {
        None => hostname::get_hostname()
            .expect("Server has no name; cannot give clients my address"),
        Some(shost) => shost.to_string(),
    };

    format!("http://{}", host)
}

impl<'r> Responder<'r> for UuidResponse {
    fn respond_to(self, req: &Request) -> Result<Response<'r>, Status> {
        match self {
            UuidResponse::Uuid {
                ref uuid,
                ref name,
                ref repo,
                ref range
            } => {
                debug!("Uuid Ok");
                let location_url = format!(
                    "{}/v2/{}/{}/blobs/uploads/{}?query=true",
                    get_base_url(req),
                    name,
                    repo,
                    uuid
                );
                let &(left, right) = range;
                let upload_uuid = Header::new("Docker-Upload-UUID", uuid.clone());
                let range = Header::new("Range", format!("{}-{}", left, right));
                let length = Header::new("X-Content-Length", format!("{}", right - left));
                let location = Header::new("Location", location_url);

                debug!("Range: {}-{}, Length: {}", left, right, right - left);
                Response::build()
                .header(upload_uuid)
                .header(location)
                .header(range)
                .header(length)
                // TODO: move into the type so it is better encoded?...
                .status(Status::Accepted)
                .ok()
            }
            UuidResponse::Empty => {
                debug!("Uuid Error");
                Response::build().status(Status::NotFound).ok()
            }
        }
    }
}

#[cfg(test)]
mod test {
    use rocket::http::Status;
    use response::uuid::UuidResponse;

    use response::test_helper::test_route;
    fn build_response() -> UuidResponse {
        UuidResponse::Uuid {
            // TODO: keep this as a real Uuid!
            uuid: String::from("whatever"),
            name: String::from("moredhel"),
            repo: String::from("test"),
            range: (0,0)
        }
    }

    #[test]
    fn uuid_uuid() {
        let response = test_route(build_response());
        let headers = response.headers();
        assert_eq!(response.status(), Status::Accepted);
        assert!(headers.contains("Docker-Upload-UUID"));
        assert!(headers.contains("Location"));
        assert!(headers.contains("Range"));
    }

    #[test]
    fn uuid_empty() {
        let response = test_route(UuidResponse::Empty);
        assert_eq!(response.status(), Status::NotFound);
    }
}
