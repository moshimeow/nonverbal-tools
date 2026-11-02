-- Read-only inspection: safe while the user changes window focus.
tell application "System Events" to tell process "speak-pocket"
 set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
 if not (exists button "Menu" of group 1 of root) then error "Missing menu"
 if not (exists button "Smaller text" of group 1 of root) then error "Missing text-size controls"
 if not (exists button "Larger text" of group 1 of root) then error "Missing text-size controls"
 if not (exists text area "Message" of group "Conversation" of root) then error "Missing unified composer"
 if not ((exists button "Speak message" of group "Conversation" of root) or (exists button "Send message" of group "Conversation" of root)) then error "Missing mode-aware send button"
end tell
return "PASS: unified composer, mode-aware send, menu, text-size controls (read-only)"
