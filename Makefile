# Version info.
VERSION_SUFFIX	:=	dev
VERSION		:=	0.6.0${VERSION_SUFFIX}
BUILD_DATE	:=	$(shell TZ=UTC date)
BUILD_DATE_ISO	:=	$(shell TZ=UTC date +%Y%m%d%H%M%S)
BUILD_REV	:=	$(shell git rev-parse --short HEAD)

LDFLAGS :=	-X \"github.com/jasonish/evebox/core.BuildDate=$(BUILD_DATE)\" \
		-X \"github.com/jasonish/evebox/core.BuildRev=$(BUILD_REV)\" \
		-X \"github.com/jasonish/evebox/core.BuildVersion=$(VERSION)\" \

# Tags required to build with sqlite - cgo must also be enabled.
TAGS :=		'fts5 json1'

ifdef WITH_SQLITE
CGO_ENABLED :=	1
else
CGO_ENABLED ?= 	0
endif

APP :=		evebox

GOOS ?=		$(shell go env GOOS)
GOARCH ?=	$(shell go env GOARCH)

WEBAPP_SRCS :=	$(shell find webapp/src -type f)
GO_SRCS :=	$(shell find . -name \*.go)

RESOURCES :=	$(shell find resources | grep -v bindata.go)

GO_PACKAGES :=	$(shell go list ./... | grep -v /vendor/)

all: public evebox

install-deps:
# NPM
	$(MAKE) -C webapp $@
# Go
	which glide > /dev/null 2>&1 || go get github.com/Masterminds/glide
	which gin > /dev/null 2>&1 || go get github.com/codegangsta/gin
	which reflex > /dev/null 2>&1 || go get github.com/cespare/reflex
	which go-bindata > /dev/null 2>&1 || go get github.com/jteeuwen/go-bindata/...
	glide install

clean:
	rm -rf dist
	rm -f evebox
	rm -f public/*.js
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf vendor
	$(MAKE) -C webapp $@

.PHONY: dist rpm deb

# Build the webapp bundle.
resources/public/bundle.js: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
public: resources/public/bundle.js

resources/bindata.go: $(RESOURCES) resources/public/bundle.js
	go generate ./resources/...

# Build's EveBox for the host platform.
evebox: Makefile $(GO_SRCS) resources/bindata.go
	CGO_ENABLED=$(CGO_ENABLED) go build --tags $(TAGS) -ldflags "$(LDFLAGS)" -o ${APP} cmd/evebox.go

# Format all go source code except in the vendor directory.
gofmt:
	@go fmt $(GO_PACKAGES)

build-with-docker:
	docker build --rm -t evebox/builder - < Dockerfile
	docker run --rm -it \
		-v `pwd`:/go/src/github.com/jasonish/evebox \
		-w /go/src/github.com/jasonish/evebox \
		evebox/builder make install-deps all

release-with-docker:
	docker build --rm -t evebox/builder - < Dockerfile
	docker run --rm -it \
		-v `pwd`:/go/src/github.com/jasonish/evebox \
		-w /go/src/github.com/jasonish/evebox \
		evebox/builder make install-deps release deb rpm

dev-server: evebox
	@if [ "${ELASTICSEARCH_URL}" = "" ]; then \
		echo "error: ELASTICSEARCH_URL not set."; \
		exit 1; \
	fi
	./webapp/node_modules/.bin/concurrently -k \
		"make -C webapp start" \
		"make dev-server-reflex" \

# Helper for dev-server mode, watches evebox Go source and rebuilds and
# restarts as needed.
dev-server-reflex: evebox
	reflex -s -R 'bindata\.go' -r '\.go$$' -- \
		sh -c "make evebox && ./evebox --dev http://localhost:8080"

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist: DISTNAME ?= ${APP}$(DIST_SUFFIX)-${VERSION}-${GOOS}-${GOARCH}
dist: LDFLAGS += -s -w
dist: CGO_ENABLED ?= $(CGO_ENABLED)
dist: resources/public/bundle.js resources/bindata.go
	CGO_ENABLED=$(CGO_ENABLED) GOARCH=$(GOARCH) GOOS=$(GOOS) \
		go build -tags $(TAGS) -ldflags "$(LDFLAGS)" \
		-o dist/$(DISTNAME)/${APP} cmd/evebox.go
	cd dist && zip -r ${DISTNAME}.zip ${DISTNAME}

release:
	GOOS=linux GOARCH=amd64 $(MAKE) dist
	CGO_ENABLED=1 DIST_SUFFIX="-sqlite" GOOS=linux GOARCH=amd64 $(MAKE) dist
	GOOS=linux GOARCH=386 $(MAKE) dist
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
		-t deb \
		-p dist \
		-n evebox \
		--epoch $(EPOCH) \
		-v $(VERSION)$(TILDE) \
		--after-upgrade=deb/after-upgrade.sh \
		dist/${APP}-${VERSION}-linux-amd64/evebox=/usr/bin/evebox \
		deb/evebox.default=/etc/default/evebox \
		deb/evebox.service=/lib/systemd/system/evebox.service

	fpm -s dir \
		-t deb \
		-p dist \
		-n evebox \
		--epoch $(EPOCH) \
		-v $(VERSION)$(TILDE) \
		--after-upgrade=deb/after-upgrade.sh \
		-a i386 \
		dist/${APP}-${VERSION}-linux-386/evebox=/usr/bin/evebox \
		deb/evebox.default=/etc/default/evebox \
		deb/evebox.service=/lib/systemd/system/evebox.service

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
		--after-upgrade=rpm/after-upgrade.sh \
		--iteration $(RPM_ITERATION) \
		--config-files /etc/sysconfig/evebox \
		dist/${APP}-${VERSION}-linux-amd64/evebox=/usr/bin/evebox \
		rpm/evebox.sysconfig=/etc/sysconfig/evebox \
		rpm/evebox.service=/lib/systemd/system/evebox.service
