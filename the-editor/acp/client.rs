use std::{
  cell::RefCell,
  rc::Rc,
};

use agent_client_protocol as acp;
use async_trait::async_trait;

/// Implementation of the ACP Client trait for the editor
/// This handles requests from the agent (file operations, permissions, etc.)
pub struct EditorClient {
  /// Queue for sending session notifications back to the editor
  notifications: Rc<RefCell<Vec<super::SessionNotification>>>,
}

impl EditorClient {
  pub fn new(notifications: Rc<RefCell<Vec<super::SessionNotification>>>) -> Self {
    Self { notifications }
  }
}

#[async_trait(?Send)]
impl acp::Client for EditorClient {
  async fn request_permission<'a>(
    &'a self,
    _args: acp::RequestPermissionRequest,
  ) -> Result<acp::RequestPermissionResponse, acp::Error> {
    // TODO: Implement permission request UI
    log::info!("Permission request received: {:?}", _args);
    Err(acp::Error::method_not_found())
  }

  async fn write_text_file<'a>(
    &'a self,
    args: acp::WriteTextFileRequest,
  ) -> Result<acp::WriteTextFileResponse, acp::Error> {
    log::info!("ACP Client: write_text_file request: {:?}", args.path);

    // Write the file to disk
    // TODO: Later, integrate with editor's document system if file is open
    match tokio::fs::write(&args.path, &args.content).await {
      Ok(()) => {
        log::info!("ACP Client: Successfully wrote file: {:?}", args.path);
        Ok(acp::WriteTextFileResponse { meta: None })
      },
      Err(err) => {
        log::error!("ACP Client: Failed to write file {:?}: {}", args.path, err);
        Err(acp::Error::internal_error())
      },
    }
  }

  async fn read_text_file<'a>(
    &'a self,
    args: acp::ReadTextFileRequest,
  ) -> Result<acp::ReadTextFileResponse, acp::Error> {
    log::info!("ACP Client: read_text_file request: {:?}", args.path);

    // Read the file from disk
    // TODO: Later, check if file is open in editor and read from document
    match tokio::fs::read_to_string(&args.path).await {
      Ok(content) => {
        log::info!("ACP Client: Successfully read file: {:?} ({} bytes)", args.path, content.len());
        Ok(acp::ReadTextFileResponse {
          content,
          meta: None,
        })
      },
      Err(err) => {
        log::error!("ACP Client: Failed to read file {:?}: {}", args.path, err);
        Err(acp::Error::internal_error())
      },
    }
  }

  async fn session_notification<'a>(
    &'a self,
    args: acp::SessionNotification,
  ) -> Result<(), acp::Error> {
    log::info!("ACP Client: session_notification - session_id: {:?}, update type: {:?}",
               args.session_id, std::mem::discriminant(&args.update));

    // Push the notification to the queue for processing in the main loop
    self.notifications.borrow_mut().push(super::SessionNotification {
      session_id: args.session_id.clone(),
      update:     args.update,
    });

    log::info!("ACP Client: Notification queued, {} total in queue",
               self.notifications.borrow().len());
    Ok(())
  }
}
