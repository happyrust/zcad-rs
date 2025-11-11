use std::collections::HashMap;

use crate::scene::Scene;

#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandResponse {
    pub success: bool,
    pub message: Option<String>,
}

impl CommandResponse {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
        }
    }
}

pub trait CommandHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        request: &CommandRequest,
        context: &mut CommandContext<'_>,
    ) -> CommandResponse;
}

pub struct CommandContext<'a> {
    pub scene: &'a mut Scene,
}

pub struct CommandBus {
    handlers: HashMap<&'static str, Box<dyn CommandHandler>>,
}

impl CommandBus {
    pub fn new() -> Self {
        let mut bus = Self {
            handlers: HashMap::new(),
        };
        bus.register(FocusSelectionCommand);
        bus.register(ClearSelectionCommand);
        bus
    }

    pub fn register<H: CommandHandler + 'static>(&mut self, handler: H) {
        self.handlers.insert(handler.name(), Box::new(handler));
    }

    pub fn dispatch(
        &self,
        request: &CommandRequest,
        context: &mut CommandContext<'_>,
    ) -> CommandResponse {
        if let Some(handler) = self.handlers.get(request.name.as_str()) {
            handler.execute(request, context)
        } else {
            CommandResponse::err(format!("未知命令: {}", request.name))
        }
    }

    pub fn available_commands(&self) -> impl Iterator<Item = &&'static str> {
        self.handlers.keys()
    }
}

struct FocusSelectionCommand;

impl CommandHandler for FocusSelectionCommand {
    fn name(&self) -> &'static str {
        "focus_selection"
    }

    fn execute(
        &self,
        _request: &CommandRequest,
        context: &mut CommandContext<'_>,
    ) -> CommandResponse {
        context.scene.focus_on_selection();
        CommandResponse::ok("视口已聚焦当前选中实体")
    }
}

struct ClearSelectionCommand;

impl CommandHandler for ClearSelectionCommand {
    fn name(&self) -> &'static str {
        "clear_selection"
    }

    fn execute(
        &self,
        _request: &CommandRequest,
        context: &mut CommandContext<'_>,
    ) -> CommandResponse {
        context.scene.clear_selection();
        CommandResponse::ok("选中集已清空")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn focus_and_clear_commands_work() {
        let mut scene = Scene::new();
        let ids = scene.populate_demo();
        scene.select(ids.circle).unwrap();

        let bus = CommandBus::new();
        let mut context = CommandContext { scene: &mut scene };

        let focus = CommandRequest {
            name: "focus_selection".to_string(),
            args: Vec::new(),
        };
        let response = bus.dispatch(&focus, &mut context);
        assert!(response.success);

        let clear = CommandRequest {
            name: "clear_selection".to_string(),
            args: Vec::new(),
        };
        let response = bus.dispatch(&clear, &mut context);
        assert!(response.success);
        assert_eq!(context.scene.selection_len(), 0);
    }
}
