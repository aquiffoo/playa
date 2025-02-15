# 🏖️ playa-rs

playa is an audio/video processing tool that supports playing audio files and rendering video files using ffmpeg's caca output

<video controls>
    <source src="./demo.mp4" type="video/mp4">
    Your browser does not support the video tag.
</video>

## Features
- play audio (mp3, wav, etc.)
- play video (mp4, mov, avi)
- change audio speed

## Usage
run the application by providing a file path as the first argv:
```sh
cargo run [file_path]
```

## Dependencies
- rodio
- cpal
- [ffmpeg](https://www.ffmpeg.org/)

## License
This project is licensed under the MIT License.
