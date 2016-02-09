GOHOSTARCH :=	$(shell go env GOHOSTARCH)
GOHOSTOS :=	$(shell go env GOHOSTOS)

export GO15VENDOREXPERIMENT=1
export CGO_ENABLED=0

VERSION :=	$(shell date +%Y%m%d%H%M)
BUILD_DATE :=	$(shell TZ=UTC date)
LDFLAGS :=	-X \"main.buildDate=$(BUILD_DATE)\"

APP :=		evebox

WEBPACK :=	./node_modules/.bin/webpack

all: public evebox

install-deps:
# NPM
	npm install
# Go
	go get github.com/Masterminds/glide
	go get github.com/GeertJohan/go.rice/rice
	glide install

clean:
	rm -rf dist
	rm -f evebox
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf node_modules vendor

.PHONY: public evebox dist

# Build the webapp bundle.
public:
	$(WEBPACK) --optimize-minimize

evebox:
	go build -ldflags "$(LDFLAGS)"

with-docker:
	docker build --rm -t evebox-builder .
	docker run --rm -it \
		-v `pwd`:/gopath/src/github.com/jasonish/evebox \
		-w /gopath/src/github.com/jasonish/evebox \
		evebox-builder make install-deps all

dev-server:
	@if [ "${EVEBOX_ELASTICSEARCH_URL}" = "" ]; then \
		echo "error: EVEBOX_ELASTICSEARCH_URL not set."; \
		exit 1; \
	fi
	./node_modules/.bin/concurrent -k "npm run server" \
		"./evebox --dev http://localhost:8080 \
		 -e ${EVEBOX_ELASTICSEARCH_URL}"

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist:
	go build -ldflags "$(LDFLAGS)" -o dist/${APP}-${GOOS}-${GOARCH}/${APP}
	rice -v append --exec dist/${APP}-${GOOS}-${GOARCH}/${APP}
	cd dist && zip -r ${APP}-${GOOS}-${GOARCH}.zip ${APP}-${GOOS}-${GOARCH}

release:
	GOOS=linux GOARCH=amd64 $(MAKE) dist
	GOOS=freebsd GOARCH=amd64 $(MAKE) dist
	GOOS=darwin GOARCH=amd64 $(MAKE) dist
	GOOS=windows GOARCH=amd64 $(MAKE) dist

deb:
	fpm -s dir \
		-C dist/evebox-linux-amd64 \
		-t deb \
		-p dist \
		-n evebox \
		-v $(VERSION) \
		--prefix /usr/bin \
		evebox

rpm:
	fpm -s dir \
		-C dist/evebox-linux-amd64 \
		-t rpm \
		-p dist \
		-n evebox \
		-v $(VERSION) \
		--prefix /usr/bin \
		evebox
