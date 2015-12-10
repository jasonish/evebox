WEBPACK :=	./node_modules/.bin/webpack

all: public
	cd backend && make

install-deps:
	npm install
	$(MAKE) -C backend install-deps

clean:
	rm -rf dist
	cd backend && make clean
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf node_modules
	cd backend && make distclean

.PHONY: public public-nominimize

public:
	$(WEBPACK) --optimize-minimize

regen-public: public
	git add public/bundle.js
	git commit public/bundle.js -m 'regen public'

public-nominimize:
	$(WEBPACK)

with-docker:
	docker build --rm -t evebox-builder .
	docker run --rm -it -v `pwd`:/gopath/src/github.com/jasonish/evebox \
		evebox-builder bash -c \
		'cd /gopath/src/github.com/jasonish/evebox && make dep all'

dev-server:
	@if [ "${EVEBOX_ELASTICSEARCH_URL}" = "" ]; then \
		echo "error: EVEBOX_ELASTICSEARCH_URL not set."; \
		exit 1; \
	fi
	./node_modules/.bin/concurrent -k "npm run server" \
		"./backend/evebox --dev http://localhost:8080 \
		 -e ${EVEBOX_ELASTICSEARCH_URL}"

deb:
	fpm -s dir \
		-C backend \
		-t deb \
		-n evebox \
		-v 0.5.1-1 \
		--prefix /usr/bin \
		evebox
