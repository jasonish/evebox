# Get the git revision if not provided in the environment.
BUILD_REV ?=	$(shell git rev-parse --short HEAD)

BUILD_ARGS +=	--aot
BUILD_ARGS +=	--configuration production
OUTPUT_PATH =	../resources/public

all: install-deps build

gitrev:
	echo "export const GITREV = \"$(BUILD_REV)\";" > src/environments/gitrev.ts

build: gitrev
	@echo "Building EveBox webapp rev $(BUILD_REV)."
	npm run ng -- build $(BUILD_ARGS) --output-path $(OUTPUT_PATH)

./node_modules/_done: package.json
	npm ci --prefer-offline
	touch $@
install-deps: ./node_modules/_done

clean:
	rm -rf node_modules
	rm -rf typings

fmt:
	npx prettier --write .
