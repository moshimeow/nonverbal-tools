-- Run only against an isolated SPEAK_DATA_DIR test instance.
on sendWrite(message, shifted)
 tell application "System Events"
  tell process "speak-pocket"
   set frontmost to true
   set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
   click radio button "Write" of tab group "View" of group 1 of root
   delay 0.2
   click text area "Write message" of group "Write" of root
   if value of text area "Write message" of group "Write" of root is not "" then error "Existing draft; refusing to overwrite"
   if not frontmost then error "App not focused"
   if not (value of attribute "AXFocused" of text area "Write message" of group "Write" of root) then error "Field not focused"
   keystroke message
   delay 0.2
   if value of text area "Write message" of group "Write" of root is not message then error "Typing mismatch"
   if shifted then
    key code 36 using shift down
   else
    key code 36
   end if
   delay 0.8
   if value of text area "Write message" of group "Write" of root is not "" then error "Enter did not clear"
   if not (exists static text message of group "Write" of root) then
    -- Articles appear as groups in WebKit's accessibility tree.
    set found to false
    repeat with messageGroup in groups of group "Write" of root
     if exists static text message of group 1 of messageGroup then set found to true
    end repeat
    if not found then error "Sent message not visible"
   end if
  end tell
 end tell
end sendWrite
my sendWrite("First written message.", false)
my sendWrite("Second written message.", true)
return "PASS: Enter and Shift-Enter submit, clear composer, and display both messages"
