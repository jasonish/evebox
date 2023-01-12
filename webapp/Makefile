# Get the git revision if not provided in the environment.
BUILD_REV ?=	$(shell git rev-parse --short HEAD)

all:
	echo "export const GIT_REV = \"$(BUILD_REV)\";" > src/gitrev.ts
	npm ci --prefer-offline
	npm run build
	rm -rf ../resources/webapp
	cp -a dist ../resources/webapp

clean:
	rm -rf dist node_modules
