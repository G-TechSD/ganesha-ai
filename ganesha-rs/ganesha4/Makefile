# Simple Makefile for compiling factorial.c

CC = gcc
CFLAGS = -Wall -Wextra -O2
TARGET = factorial

all: $(TARGET)

$(TARGET): factorial.o
	$(CC) $(CFLAGS) -o $@ $^

factorial.o: factorial.c
	$(CC) $(CFLAGS) -c $<

clean:
	rm -f *.o $(TARGET)

.PHONY: all clean
