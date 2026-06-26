use lsp_types::{GotoDefinitionResponse, Location};
use serde::Serialize;

macro_rules! define_request {
    (
        $(
            $variant:ident $( ( $params:ident ) )? = $string:literal,
        )*
    ) => {
        pub enum Request {
            $(
                $variant (lsp_server::RequestId, $( lsp_types::$params )? ),
            )*
        }

        impl TryFrom<lsp_server::Request> for Request {
            type Error = anyhow::Error;

            fn try_from(request: lsp_server::Request) -> anyhow::Result<Self> {
                let lsp_server::Request { id, method, params } = request;
                match method.as_str() {
                    $(
                        $string => Ok(
                            Request::$variant(
                                id,
                                $(
                                    serde_json::from_value::<lsp_types::$params>(params)?,
                                )?
                            ),
                        ),
                    )*
                    method => anyhow::bail!("Unknown request method: {method:?}"),
                }
            }
        }
    };
}

macro_rules! define_response {
    (
        $(
            $variant:ident ( $params:ty ),
        )*
    ) => {
        pub enum Response {
            Ok(lsp_server::RequestId),
            $(
                $variant ( lsp_server::RequestId, $params ),
            )*
        }

        impl From<Response> for lsp_server::Response {
            fn from(response: Response) -> Self {
                match response {
                    Response::Ok(id) => lsp_server::Response {
                        id,
                        result: None,
                        error: None,
                    },
                    $(
                        Response::$variant(id, params) => lsp_server::Response {
                            id,
                            result: to_value(params),
                            error: None,
                        },
                    )*
                }
            }
        }
    };
}

macro_rules! define_notification {
    (
        $(
            $variant:ident $( ( $params:ident ) )? = $string:literal,
        )*
    ) => {
        pub enum Notification {
            $(
                $variant $( ( lsp_types::$params ) )?,
            )*
        }

        impl TryFrom<lsp_server::Notification> for Notification {
            type Error = anyhow::Error;

            fn try_from(notification: lsp_server::Notification) -> anyhow::Result<Self> {
                let lsp_server::Notification { method, params } = notification;
                match method.as_str() {
                    $(
                        $string => Ok(
                            Notification::$variant $( (
                                serde_json::from_value::<lsp_types::$params>(params)?,
                            ) )? ,
                        ),
                    )*
                    method => anyhow::bail!("Unknown notification method: {method:?}"),
                }
            }
        }

        impl From<Notification> for lsp_server::Notification {
            fn from(notification: Notification) -> Self {
                match notification {
                    $(
                        notification_pattern! ($variant, $(params: $params,)?) =>
                        notification_to_lsp! ($variant, $(params: $params,)? $string,),
                    )*
                }
            }
        }
    };
}

macro_rules! notification_pattern {
    ($variant:ident,) => {
        Notification::$variant
    };
    ($variant:ident, $params_value:ident: $params_type:ident,) => {
        Notification::$variant($params_value)
    };
}

macro_rules! notification_to_lsp {
    ($variant:ident, $string:literal,) => {
        lsp_server::Notification {
            method: $string.to_owned(),
            params: serde_json::Value::Null,
        }
    };
    ($variant:ident, $params_value:ident: $params_type:ident, $string:literal,) => {
        lsp_server::Notification {
            method: $string.to_owned(),
            params: to_value::<lsp_types::$params_type>($params_value).unwrap_or_default(),
        }
    };
}

define_request! {
    Shutdown = "shutdown",
    GotoDefinition(GotoDefinitionParams) = "textDocument/definition",
    FindReferences(ReferenceParams) = "textDocument/references",
}

define_response! {
    GotoDefinition(GotoDefinitionResponse),
    Locations(Vec<Location>),
}

define_notification! {
    Exit = "exit",
    DidOpenTextDocument(DidOpenTextDocumentParams) = "textDocument/didOpen",
    DidChangeTextDocument(DidChangeTextDocumentParams) = "textDocument/didChange",
    DidCloseTextDocument(DidCloseTextDocumentParams) = "textDocument/didClose",
    DidChangeConfiguration(DidChangeConfigurationParams) = "workspace/didChangeConfiguration",
    LogMessage(LogMessageParams) = "window/logMessage",
}

impl From<Response> for lsp_server::Message {
    fn from(response: Response) -> Self {
        lsp_server::Message::Response(response.into())
    }
}

impl From<Notification> for lsp_server::Message {
    fn from(notification: Notification) -> Self {
        lsp_server::Message::Notification(notification.into())
    }
}

fn to_value<T: Serialize>(value: T) -> Option<serde_json::Value> {
    serde_json::to_value(value)
        .inspect_err(|err| tracing::error!("Error serializing message: {err}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shutdown_from_lsp() {
        let lsp_req = lsp_server::Request {
            id: lsp_server::RequestId::from(1),
            method: "shutdown".to_owned(),
            params: serde_json::Value::Null,
        };
        assert!(matches!(
            Request::try_from(lsp_req).unwrap(),
            Request::Shutdown(_)
        ));
    }

    #[test]
    fn response_ok_to_lsp() {
        let id = lsp_server::RequestId::from(42);
        let lsp_resp = lsp_server::Response::from(Response::Ok(id.clone()));
        assert_eq!(lsp_resp.id, id);
        assert!(lsp_resp.result.is_none());
        assert!(lsp_resp.error.is_none());
    }

    #[test]
    fn notification_exit_from_lsp() {
        let lsp_notif = lsp_server::Notification {
            method: "exit".to_owned(),
            params: serde_json::Value::Null,
        };
        assert!(matches!(
            Notification::try_from(lsp_notif).unwrap(),
            Notification::Exit
        ));
    }

    #[test]
    fn notification_log_message_to_lsp() {
        let params = lsp_types::LogMessageParams {
            typ: lsp_types::MessageType::INFO,
            message: "hello".to_owned(),
        };
        let lsp_notif = lsp_server::Notification::from(Notification::LogMessage(params));
        assert_eq!(lsp_notif.method, "window/logMessage");
        assert_eq!(lsp_notif.params["message"], "hello");
    }

    #[test]
    fn to_value_string() {
        assert_eq!(
            to_value("hello"),
            Some(serde_json::Value::String("hello".into())),
        );
    }

    #[test]
    fn to_value_serialization_error_returns_none() {
        struct AlwaysFails;
        impl Serialize for AlwaysFails {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("forced error"))
            }
        }
        // Must not panic — errors are swallowed via inspect_err + ok()
        assert!(to_value(AlwaysFails).is_none());
    }
}
