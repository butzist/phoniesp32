# Toplevel justfile to build all components and create ESP binary

# Build everything and create the ESP binary
all:
	cd cli && just all
	cd firmware && just all
	cp firmware/target/xtensa-esp32-none-elf/release/firmware ./firmware.bin

# Build CLI only
cli:
	cd cli && just all
	cp cli/target/release/cli ./pecli

# Test all components
test:
	cd audio-file-utils && cargo test
	cd transcoder && just test
	cd transcoder-webworker && just test
	cd cli && just test

# Clean all sub-projects
clean:
	cd transcoder && just clean
	cd web && just clean
	cd transcoder-webworker && just clean
	cd cli && just clean
	cd firmware && just clean
	rm -f firmware.bin

flash:
	cd firmware && just flash

run:
	cd firmware && just run
