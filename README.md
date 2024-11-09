# Ten Sleep 🛌💤🔟
Control the Eight Sleep Pod 3 locally and automatically!

This project uses the Pod's existing firmware (`frankenfirmware`) but replaces the DAC and disables PizzaRat (& Capybara for now).
Once setup, you __CANNOT__ use the Eight Sleep mobile app to control the bed.

This project is build on top of work by `ninesleep`, see [Credits](#credits-).

## Features 😴
 1. Automatically set bed temperature every night
 2. Create a custom temperature profile
 3. Set a heat and/or vibration wakeup alarm
 4. (FUTURE) Get processed sleep tracking data from Capybara

## Setup 🥱
To use Ten Sleep you must disassemble the Eight Sleep Pod 3, modify the SD card's `rootfs.tar.gz`
to add your SSH key + root password, and reset the Pod. Then power the Pod while holding the small
button on the back, which performs a factory reset from `rootfs.tar.gz`. Now you can disable
Eight Sleep's update service and [Add Ten Sleep](#adding-ten-sleep-).
 - __Note__: the default SSH port for Pod 3 is `8822`.
 - __Disable Updates__: `systemctl disable --now swupdate-progress swupdate defibrillator eight-kernel telegraf vector`

Eventually I will add thorough tutorial for this, but for now I would recommend
[Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
and the [ninesleep tutorial](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions).


### Adding Ten Sleep 🔟
Once this project is more complete I will create a release containing the binary.
Until then compile this repo with:

```bash
cargo build --target aarch64-unknown-linux-musl
```

 1. `scp` the binary and service (`tensleep.service`) to the Pod
 2. `ssh` in, sign in as root
 3. Move the binary to `/opt/tensleep`
 4. Move the service file `/etc/systemd/system/multi-user.target.wants`
 5. Stop the DAC `systemctl stop dac`
 6. Enable the service `systemctl enable --now tensleep`

## Usage 🖥️
TODO

## API 🔌
TODO

## Credits 👏
This project is built on top of the work done by [ninesleep](https://github.com/bobobo1618/ninesleep).
Their license is included under [src/frank/LICENSE](src/frank/LICENSE) where a modified version of their code exists.

## Footnotes 📝
This project is not affiliated with or endorsed by Eight Sleep.

If you encounter issues with this project please make an
issue on this repository. If you have changes you want to
be made please make a PR.

For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).
