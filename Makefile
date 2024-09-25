MAKEFLAGS += --no-print-directory
VERSION=$(shell git tag -l --sort=-v:refname| sed 's/v//g'| head -n 1)
GIT_TAG=$(shell git describe --tags)
PROJECT='rustdio'

.PHONY:
.SILENT:
.DEFAULT_GOAL := help


build: ## build
	cargo build --release

flash-monitor: ## flash and monitor
	espflash flash --flash-size 16mb --monitor  --partition-table partition-table.bin --erase-parts nvs target/xtensa-esp32s3-espidf/debug/rustdio

all: ## build, flash and monitor
	make build
	make flash-monitor

version: ## display version of rustdio
	@echo $(VERSION)

clippy:
	cargo clippy --all-targets --all-features --workspace -- -D warnings

fmt:
	cargo fmt --all -- --check --color always

# Absolutely awesome: http://marmelab.com/blog/2016/02/29/auto-documented-makefile.html
help:
	$(eval PADDING=$(shell grep -x -E '^[a-zA-Z_-]+:.*?##[\s]?.*$$' Makefile | awk '{ print length($$1)-1 }' | sort -n | tail -n 1))
	clear
	echo '╔──────────────────────────────────────────────────╗'
	echo '║ ██╗  ██╗███████╗██╗     ██████╗ ███████╗██████╗  ║'
	echo '║ ██║  ██║██╔════╝██║     ██╔══██╗██╔════╝██╔══██╗ ║'
	echo '║ ███████║█████╗  ██║     ██████╔╝█████╗  ██████╔╝ ║'
	echo '║ ██╔══██║██╔══╝  ██║     ██╔═══╝ ██╔══╝  ██╔══██╗ ║'
	echo '║ ██║  ██║███████╗███████╗██║     ███████╗██║  ██║ ║'
	echo '║ ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝     ╚══════╝╚═╝  ╚═╝ ║'
	echo '╟──────────────────────────────────────────────────╝'
	@grep -E '^[a-zA-Z_-]+:.*?##[\s]?.*$$' Makefile | awk 'BEGIN {FS = ":.*?##"}; {gsub(/(^ +| +$$)/, "", $$2);printf "╟─[ \033[36m%-$(PADDING)s\033[0m %s\n", $$1, "] "$$2}'
	echo '╚──────────────────────────────────────────────────>'
	echo ''
