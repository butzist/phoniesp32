#!/usr/bin/env bash

# Get the firmware binary path
FIRMWARE="target/riscv32imac-unknown-none-elf/release/firmware"

if [ ! -f "$FIRMWARE" ]; then
  echo "Error: Firmware binary not found at $FIRMWARE"
  echo "Please build the firmware first"
  exit 1
fi

echo "=== Section Sizes (decimal) ==="
echo "Binary: $FIRMWARE"
echo

# Get section sizes using size command
if command -v size >/dev/null 2>&1; then
  echo "Section sizes:"
  size "$FIRMWARE" | awk 'NR==1 {print $0} NR>1 {printf "text: %d, data: %d, bss: %d, total: %d\n", $1, $2, $3, $4}'
  echo
else
  echo "Warning: 'size' command not available, using nm for section analysis"
fi

echo "=== Stack Analysis ==="
# Get stack start and end symbols using nm
STACK_START=$(nm "$FIRMWARE" | grep " _stack_start$" | awk '{print $1}')
STACK_END=$(nm "$FIRMWARE" | grep " _stack_end$" | awk '{print $1}')

if [ -n "$STACK_START" ] && [ -n "$STACK_END" ]; then
  # Convert hex to decimal and calculate stack size
  START_DEC=$((0x$STACK_START))
  END_DEC=$((0x$STACK_END))
  STACK_SIZE=$((START_DEC - END_DEC))

  echo "Stack start: $STACK_START (decimal: $START_DEC)"
  echo "Stack end:   $STACK_END (decimal: $END_DEC)"
  echo "Stack size:  $STACK_SIZE bytes"
else
  echo "Warning: Could not find _stack_start and/or _stack_end symbols"
  echo "Available stack-related symbols:"
  nm "$FIRMWARE" | grep -i stack | head -10
fi

