GITREV :=	$(shell git rev-parse --short HEAD)

BUILD_ARGS +=	--sourcemaps
BUILD_ARGS +=	--aot
BUILD_ARGS +=	--prod

OUTPUT_PATH =	../resources/public

NG :=		./node_modules/.bin/ng

all: build

gitrev:
	@echo "Exporting GITREV = $(GITREV)."
	@echo "export const GITREV = \"$(GITREV)\";" > src/environments/gitrev.ts

build: gitrev
	@echo "Building EveBox webapp rev $(GITREV)."
	$(NG) build $(BUILD_ARGS) --output-path $(OUTPUT_PATH)

install-deps:
	npm install

distclean:
	rm -rf node_modules
	rm -rf typings

start: gitrev
	$(NG) server $(BUILD_ARGS)

