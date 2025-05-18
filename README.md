# League Auto Accept - Rust Edition

A Rust implementation of the League Auto Accept tool that automatically accepts queue and handles champion select in League of Legends.

## Features

- Automatically accept queue
- Pick a champion
- Ban a champion
- Can instalock
- Pick summoner spells
- Send chat messages when entering lobby
- Toggle auto accept with 'T' key
- Quit application with 'Q' key

## Requirements

- Windows OS (currently only Windows is supported)
- Rust toolchain (install from [rustup.rs](https://rustup.rs/))
- League of Legends client running

## Installation

1. Clone the repository
2. Build the project:
```bash
cargo build --release
```
3. The executable will be available in `target/release/league_auto_accept.exe`

## Configuration

The application uses a configuration file located at:
- Windows: `%APPDATA%/LeagueAutoAccept/LeagueAutoAccept/config/settings.json`

Example configuration:
```json
{
  "auto_accept": true,
  "champion_select": {
    "enabled": true,
    "champion_id": 1,
    "ban_id": 2,
    "spell1_id": 4,
    "spell2_id": 14,
    "instalock": true,
    "chat_messages": ["Mid pref"]
  },
  "queue": {
    "auto_restart": false,
    "max_time": 300000
  }
}
```

## Usage

1. Start League of Legends client
2. Run the League Auto Accept executable
3. The application will automatically:
   - Accept queue when a match is found
   - Pick/ban champions based on your configuration
   - Set summoner spells
   - Send chat messages

## Controls

- Press 'T' to toggle auto accept on/off
- Press 'Q' to quit the application

## Notes

- This application uses the LCU API, which is not officially supported by Riot Games
- Use at your own risk
- Not supported on Korean servers
- Make sure to configure your settings in the configuration file before using champion select features

## License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information. 