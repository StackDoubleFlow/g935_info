# g935_info

Check to see if a Logitech G935 Wireless Headset is connected (wirelessly) and display battery information.

Can also be used with the i3bar protocol. For example, with [i3status-rust](https://github.com/greshake/i3status-rust):

```toml
[icons.overrides]
headset = " "
headset_charging = " "

[[block]]
block = "custom"
command = "g935_info get-i3-status --update-pulseaudio"
json = true
persistent = true
hide_when_empty = true
```
