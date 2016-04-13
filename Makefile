# Version info.
VERSION		:=	0.5.0
VERSION_SUFFIX	:=	dev
BUILD_DATE	:=	$(shell TZ=UTC date)
BUILD_DATE_ISO	:=	$(shell TZ=UTC date +%Y%m%d%H%M%S)
BUILD_REV	:=	$(shell git rev-parse --short HEAD)

GOHOSTARCH :=	$(shell go env GOHOSTARCH)
GOHOSTOS :=	$(shell go env GOHOSTOS)

export GO15VENDOREXPERIMENT=1

LDFLAGS :=	-X \"main.buildDate=$(BUILD_DATE)\" \
		-X \"main.buildRev=$(BUILD_REV)\" \
		-X \"main.buildVersion=$(VERSION)$(VERSION_SUFFIX)\"

APP :=		evebox

WEBPACK :=	./node_modules/.bin/webpack

WEBAPP_SRCS :=	$(shell find webapp -type f)
GO_SRCS :=	$(shell find . -name \*.go)

all: public evebox

install-deps:
# NPM
	npm install
# Go
	go get github.com/Masterminds/glide
	go get github.com/GeertJohan/go.rice/rice
	go get github.com/codegangsta/gin
	glide install

clean:
	rm -rf dist
	rm -f evebox
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf node_modules vendor

.PHONY: public dist rpm

# Build the webapp bundle.
public/bundle.js: $(WEBAPP_SRCS)
	$(WEBPACK) --optimize-minimize
public: public/bundle.js

evebox: $(GO_SRCS)
	CGO_ENABLED=0 go build -ldflags "$(LDFLAGS)"

with-docker:
	docker build --rm -t evebox/builder - < Dockerfile
	docker run --rm -it \
		-v `pwd`:/go/src/evebox \
		-w /go/src/evebox \
		evebox/builder make install-deps all

dev-server:
	@if [ "${EVEBOX_ELASTICSEARCH_URL}" = "" ]; then \
		echo "error: EVEBOX_ELASTICSEARCH_URL not set."; \
		exit 1; \
	fi
	./node_modules/.bin/concurrent -k \
		"npm run server" \
		"gin --appPort 5636 -i -b evebox ./evebox -e ${EVEBOX_ELASTICSEARCH_URL} --dev http://localhost:8080"

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist:
	CGO_ENABLED=0 go build -ldflags "$(LDFLAGS)" -o dist/${APP}-${GOOS}-${GOARCH}/${APP}
	rice -v append --exec dist/${APP}-${GOOS}-${GOARCH}/${APP}
	cd dist && zip -r ${APP}-${GOOS}-${GOARCH}.zip ${APP}-${GOOS}-${GOARCH}

release:
	GOOS=linux GOARCH=amd64 $(MAKE) dist
	GOOS=freebsd GOARCH=amd64 $(MAKE) dist
	GOOS=darwin GOARCH=amd64 $(MAKE) dist
	GOOS=windows GOARCH=amd64 $(MAKE) dist

# Debian packaging.
deb: EPOCH := 1
ifneq ($(VERSION_SUFFIX),)
deb: TILDE := ~$(VERSION_SUFFIX)$(BUILD_DATE_ISO)
endif
deb:
	fpm -s dir \
		-C dist/evebox-linux-amd64 \
		-t deb \
		-p dist \
		-n evebox \
		--epoch $(EPOCH) \
		-v $(VERSION)$(TILDE) \
		--prefix /usr/bin \
		evebox

# RPM packaging.
ifneq ($(VERSION_SUFFIX),)
rpm: RPM_ITERATION := 0.$(BUILD_DATE_ISO)
else
rpm: RPM_ITERATION := 1
endif
rpm:
	fpm -s dir \
		-t rpm \
		-p dist \
		-n evebox \
		-v $(VERSION) \
		--iteration $(RPM_ITERATION) \
		--config-files /etc/sysconfig/evebox \
		dist/evebox-linux-amd64/evebox=/usr/bin/evebox \
		rpm/evebox.sysconfig=/etc/sysconfig/evebox \
		rpm/evebox.service=/lib/systemd/system/evebox.service
