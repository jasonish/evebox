all: build

build:
	./node_modules/.bin/webpack --config webpack.prod.js --progress --profile --bail

install-deps:
	npm install

distclean:
	rm -rf node_modules
	rm -rf typings

start:
	npm run start
