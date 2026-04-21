## TODO things that I'll just throw here so I don't forget

- We need a proper banner for the swift app and remove the idea that we need to have at least one buffer open, specifically in the swift app
- Theme switching is painfully slow
- File tree:
  - Add double clicking behavior 
- Ghostty:
  - Scrollbar
  - Command + F working properly
  - Scrolling when I select and go to the top/bottom of the viewport - Dragging.
- Inline completion rendering - It would be nice to make something like zed here, including all of that complicated system, it would be very cool if we had that complicated jumps zed supports. Not sure if it's model specific tho, doesn't seem like it.
- "gw" does not render anything
- Fix signature helper triggering an error every time I enter insert mode in a buffer that does not support it
- Completely overhaul the notification system for the swift app, also include system notifications for when agents are doing their thing
- We probably need some sort of VCS integrations for ease of reviewing AI generated code. not sure what the UX for that would look like tho. VCS picker doesn't feel as nice I guess, but of course I could just improve it or "fix" it
- Configuration support - We really only care for simple things for now: default theme, default font family, font size, cursor shape, whatever, not that deep
- "Fix" all pickers - Low prio tbh I don't use most of them 
- Horizontally scrollable pane tabs
- Font resizing
- Drag surfaces around to reorder them
- LSP hover on actual cursor hover
- Adhere to swiftUI/macOS defaults 
- Things need to take the :pwd command seriously (ghostty for sure and file tree likely)

Agent panel: 
- Performance is still not ideal, particularly when text is streaming and I try to do other things like fuck around with the terminal 
- Theme updates are not changing markdown rendered colors
- We need to create the cmd + L binding that basically just inputs a selection into the text agent prompt - However if we have multiple agent panels, which one will it be inputed into? Probably the correct behavior would be the one in focus? 
- Render Pi statusline, same as what we do with the terminal panes
- Agent-follow command is going to be dope.
- Proper rendered diffs with syntax highlighting? 
- Diff capabilities need to be both global and agent panel specific. We need the agent panel to be able to tell us what changes it has made on one specific pass (kinda like a summary if you will) but also need the-editor to swiftly tell you about all changes made VCS wise
- Add better support for the other commands:
  - /compact -> actually render the compaction
  - /tree    -> full support 
