# PhonieSP32 API Documentation

This document describes the REST API endpoints provided by the PhoniESP32
firmware web server.

## Base URL

All endpoints are relative to `http://<device-ip>`

**Default IP Address:** `192.168.42.1` (when device is in Access Point mode)

**Default WiFi Credentials:**

- SSID: `phoniesp32`
- Password: `12345678`

## Data Types

### AudioMetadata

```json
{
  "artist": "string (max 31 chars)",
  "title": "string (max 31 chars)",
  "album": "string (max 31 chars)",
  "duration": "number (seconds)"
}
```

### FileEntry

```json
{
  "name": "string (max 8 chars)",
  "metadata": "AudioMetadata"
}
```

### Association

```json
{
  "fob": "string (max 8 chars)",
  "files": ["FileEntry"]
}
```

### LastFob

```json
{
  "last_fob": "string (max 8 chars) or null"
}
```

### StatusResponse

```json
{
  "position_seconds": "number",
  "state": "Playing|Paused|Stopped",
  "index_in_playlist": "number",
  "playlist_name": "string (max 8 chars) or null"
}
```

### CurrentPlaylistResponse

```json
{
  "playlist_name": "string (max 8 chars)",
  "files": [{
    "file": "string (max 8 chars)",
    "metadata": "AudioMetadata"
  }]
}
```

### DeviceConfig

```json
{
  "ssid": "string",
  "password": "string"
}
```

## Endpoints

### Files

#### GET /api/files

List all audio files on the device.

**Response:** Array of `FileEntry`

#### GET /api/files/{filename}

Get metadata for a specific audio file.

**Parameters:**

- `filename`: string (max 8 chars, without .wav extension)

**Response:** `AudioMetadata` or 404 if not found

#### POST /api/files/{filename}

Create an empty audio file for chunked upload.

**Parameters:**

- `filename`: string (max 8 chars, without .wav extension)

**Response:** 201 Created on success

#### PATCH /api/files/{filename}

Upload a chunk of data to an existing audio file.

**Parameters:**

- `filename`: string (max 8 chars, without .wav extension)

**Headers:**

- `Upload-Offset`: number - Byte offset to start writing at

**Request Body:** Raw audio file chunk data

**Response:** 204 No Content on success, 404 if file doesn't exist, 400 if offset is larger than file size

#### HEAD /api/files/{filename}

Get the current size of an audio file.

**Parameters:**

- `filename`: string (max 8 chars, without .wav extension)

**Response Headers:**

- `Upload-Offset`: number - Current file size in bytes

**Response:** 200 OK on success, 404 if file doesn't exist

#### PUT /api/files/{filename}

Upload an entire audio file to the device (legacy endpoint).

**Parameters:**

- `filename`: string (max 8 chars, without .wav extension)

**Request Body:** Raw audio file data (WAV format, 44100 kHz, IMA ADPCM). Use
transcoder!

**Response:** 204 No Content on success

### Chunked Upload Workflow

For reliable file uploads with resume capability:

1. **Create file:** `POST /api/files/{filename}` - Creates empty file
2. **Upload chunks:** `PATCH /api/files/{filename}` with `Upload-Offset` header - Upload data in chunks
3. **On error:** Check progress with `HEAD /api/files/{filename}` and resume from last valid offset
4. **Repeat steps 2-3** until upload complete

**Example chunked upload with error handling:**
```bash
# Create empty file
curl -X POST http://192.168.42.1/api/files/song1

# Upload first chunk
curl -X PATCH http://192.168.42.1/api/files/song1 \
  -H "Upload-Offset: 0" \
  --data-binary @chunk1.bin

# Upload second chunk (simulating network error)
curl -X PATCH http://192.168.42.1/api/files/song1 \
  -H "Upload-Offset: 1024" \
  --data-binary @chunk2.bin || echo "Upload failed, checking progress..."

# Check current file size to resume from
CURRENT_SIZE=$(curl -I http://192.168.42.1/api/files/song1 | grep Upload-Offset | cut -d' ' -f2 | tr -d '\r')
echo "Current file size: $CURRENT_SIZE bytes"

# Resume upload from current size
curl -X PATCH http://192.168.42.1/api/files/song1 \
  -H "Upload-Offset: $CURRENT_SIZE" \
  --data-binary @chunk2.bin

# Continue with remaining chunks...
```

### Associations (RFID FOBs)

#### GET /api/last_fob

Get the last scanned RFID FOB.

**Response:** `LastFob`

#### GET /api/associations

List all FOB associations.

**Query Parameters:**

- `fob` (optional): Filter by specific FOB name

**Response:**

- Without `fob` parameter: Array of `Association`
- With `fob` parameter: Single `Association` or 404 if not found

#### POST /api/associations

Create a new FOB association.

**Request Body:**

```json
{
  "fob": "string (max 8 chars)",
  "files": ["string (max 8 chars)"]
}
```

**Response:** 204 No Content on success

### Playback Control

#### GET /api/playback/status

Get current playback status.

**Response:** `StatusResponse`

#### GET /api/playback/current_playlist

Get current playlist with metadata.

**Response:** `CurrentPlaylistResponse` or null if not playing

#### POST /api/playback/play

Start playback.

**Request Body:**

```json
{
  "file": "string (max 8 chars)", // Play single file
  // OR
  "playlist": ["string (max 8 chars)"], // Play list of files
  // OR
  "playlistref": "string (max 8 chars)" // Play playlist by FOB name
}
```

**Response:** 204 No Content on success

#### POST /api/playback/stop

Stop playback.

**Response:** 204 No Content on success

#### POST /api/playback/pause

Toggle pause/play state.

**Response:** 204 No Content on success

#### POST /api/playback/volume_up

Increase volume.

**Response:** 204 No Content on success

#### POST /api/playback/volume_down

Decrease volume.

**Response:** 204 No Content on success

#### POST /api/playback/next

Skip to the next track in the current playlist.

**Response:** 204 No Content on success

#### POST /api/playback/previous

Skip to the previous track in the current playlist.

**Response:** 204 No Content on success

### Configuration

#### PUT /api/config

Update device configuration (WiFi settings).

**Request Body:** `DeviceConfig`

**Response:** 204 No Content on success

#### DELETE /api/config

Reset device configuration.

**Response:** 204 No Content on success

## Notes

- All string fields have maximum length limits as specified
- Audio files need to be transcoded with the Web UI or standalone transcoder
  tool in this project
- File names are limited to 8 characters (DOS 8.3 naming without extension)
- FOB names are also limited to 8 characters
- The device serves a web interface that uses these API endpoints
