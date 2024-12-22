# Ten Sleep 🛌💤🔟

Control the Eight Sleep Pod 3 locally and automatically!

Ten Sleep communicates with the bed's firmware (`frakenfirmware`) by pretending
to be the DAC. This means that, once setup, you __CANNOT__ use the Eight Sleep
mobile app to control the bed.

## Features 😴
 1. Automatically set bed temperature every night
 2. Create a custom temperature profile
 3. Set a heat and/or vibration wakeup alarm
 4. Control settings and monitor remotely via API

## Setup 🥱
To use Ten Sleep you must disassemble the Eight Sleep Pod 3, modify the SD card's `rootfs.tar.gz`
to add your SSH key + root password, and reset the Pod. Then power the Pod while holding the small
button on the back, which performs a factory reset from `rootfs.tar.gz`. Now you can disable
Eight Sleep's update service and [Add Ten Sleep](#adding-ten-sleep-).
 - __Note__: the default SSH port for Pod 3 is `8822`.
 - __Disable Updates__: `systemctl disable --now swupdate-progress swupdate defibrillator eight-kernel telegraf vector`

Eventually I will add thorough tutorial for this, but for now I would recommend
[Bo Lopker's Tutorial](https://blopker.com/writing/04-zerosleep-1/#disassembly-overview)
and the [ninesleep instructions](https://github.com/bobobo1618/ninesleep?tab=readme-ov-file#instructions).

### Adding Ten Sleep 🔟

Build with:

```bash
cargo build --target aarch64-unknown-linux-musl
```

 1. Modify `settings.json` to suit your needs
 2. `scp` the binary, `tensleep.service`, and `settings.json` to the Pod
 3. `ssh` in, sign in as root
 4. Move the binary and json to `/opt/tensleep`
 5. Move the service file to `/etc/systemd/system`
 6. Stop the DAC `systemctl disable --now dac`
 7. Enable the Ten Sleep `systemctl enable --now tensleep`

## API 🔌

The entire API is JSON based.

### Errors
Some endpoints can throw an error (labeled Errorable) which has the standard type:
```json
{"error": ErrorString, "details": DetailsString}
```
(details is optional).

### Health
`GET /health` →
```json
{"status": "OK" | "UNAVAILABLE"}
```
Returns whether it can communicate to frakenfirmware

### State
`GET /state` (Errorable) →
```json
{
    "target_heat_level_left": i32,
    "target_heat_level_right": i32,
    "heat_level_left": i32,
    "heat_level_right": i32,
    "heat_time_left": u32,
    "heat_time_right": u32,
    "sensor_label": String,
    "water_level": bool,
    "priming": bool,
    "settings": String,
}
```
Returns state/variables directly from frakenfirmware.
`sensor_label` looks something like `20600-0001-F00-0001089C`,
and `settings` looks something like `BF61760162676C190190626772190190626C621864FF`


### Prime
`GET|POST /prime` (Errorable) →
```json
{"response": ResponseString}
```
Tells frakenfirmware to prime the bed. This process can take awhile.
You can check when its done with `GET /state`.priming.

### Settings
__Settings Format__:
```json
{
    "temp_profile": [i32; 3],
    "time_zone": String,
    "sleep_time": String,
    "alarm": {
        "time": TimeString,
        "vibration":{
            "pattern": "double" | "rise",
            "intensity": u8,
            "duration": u16,
            "offset": u16
        },
        "heat": {
            "temp": i32,
            "offset": u16
        }
    }
}
```

`GET /settings` (Errorable) → all settings

`GET /setting/SETTING+` (Errorable) → specified setting
 - Example: `GET /setting/alarm/vibration/duration`

`POST /settings` (Errorable): set all settings
→
```json
{ "message": "Settings updated successfully", "settings": NewSettings }
```

`POST /setting/SETTING+` (Errorable): set specified setting
→
```json
{ "message": "Setting updated successfully", "settings": NewSettings }
```
 - Example: `POST /setting/alarm/vibration/duration -d 600`






## Credits 👏
This project was inspired by [ninesleep](https://github.com/bobobo1618/ninesleep).
I have completely rewritten all of bobobo1618's code so the license is now
excluded.

## Footnotes 📝
This project is not affiliated with or endorsed by Eight Sleep.

If you encounter issues with this project please make an
issue on this repository. If you have changes you want to
be made please make a PR.

For anything else please contact me at [mail@liamsnow.com](mailto:mail@liamsnow.com).
