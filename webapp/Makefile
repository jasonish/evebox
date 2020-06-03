BUILD_REV ?=	$(shell git rev-parse --short HEAD)

# Issues with Angular 5.
#BUILD_ARGS +=	--sourcemaps
BUILD_ARGS +=	--aot
BUILD_ARGS +=	--prod

ifdef NO_PROGRESS
BUILD_ARGS +=	--progress=false
endif

OUTPUT_PATH =	../resources/public

NG :=		./node_modules/.bin/ng

PROGRESS :=	$(shell test -t || "--no-progress")

all: install-deps build

gitrev:
	@echo "Exporting GITREV = $(BUILD_REV)."
	@echo "export const GITREV = \"$(BUILD_REV)\";" > src/environments/gitrev.ts

build: gitrev
	@echo "Building EveBox webapp rev $(BUILD_REV)."
	$(NG) build $(BUILD_ARGS) --output-path $(OUTPUT_PATH) --deploy-url "public/" $(PROGRESS)

./node_modules/_done: package.json
	npm ci --prefer-offline
	touch $@

install-deps: ./node_modules/_done

clean:
	rm -rf node_modules
	rm -rf typings

serve: gitrev
	npm serve

