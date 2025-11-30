# Toplevel justfile to build all components and create ESP binary

# Build everything and create the ESP binary
all:
	cd firmware && just all
	cp firmware/target/xtensa-esp32-none-elf/release/firmware ./firmware.bin

# Clean all sub-projects
clean:
	cd transcoder && just clean
	cd web && just clean
	cd transcoder-webworker && just clean
	cd firmware && just clean
	rm -f firmware.bin

flash:
	cd firmware && just flash

run:
	cd firmware && just run
