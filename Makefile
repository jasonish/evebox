# Version info.
VERSION_SUFFIX	:=	dev
VERSION		:=	0.6.0${VERSION_SUFFIX}
BUILD_DATE	?=	$(shell TZ=UTC date)
BUILD_DATE_ISO	?=	$(shell TZ=UTC date +%Y%m%d%H%M%S)
export BUILD_DATE
export BUILD_DATE_ISO
BUILD_REV	:=	$(shell git rev-parse --short HEAD)

LDFLAGS :=	-X \"github.com/jasonish/evebox/core.BuildDate=$(BUILD_DATE)\" \
		-X \"github.com/jasonish/evebox/core.BuildRev=$(BUILD_REV)\" \
		-X \"github.com/jasonish/evebox/core.BuildVersion=$(VERSION)\" \

ifdef WITH_SQLITE
CGO_ENABLED :=	1
TAGS +=		fts5 json1
else
CGO_ENABLED ?= 	0
endif

APP :=		evebox

GOOS ?=		$(shell go env GOOS)
GOARCH ?=	$(shell go env GOARCH)

GO_SRCS :=	$(shell find . -name \*.go | grep -v /vendor/)
GO_PACKAGES :=	$(shell go list ./... | grep -v /vendor/)

WEBAPP_SRCS :=	$(shell find webapp/src -type f)

all: public evebox

install-deps:
# NPM
	$(MAKE) -C webapp $@
# Go
	which glide > /dev/null 2>&1 || \
		go get github.com/Masterminds/glide
	which reflex > /dev/null 2>&1 || \
		go get github.com/cespare/reflex
	which go-bindata > /dev/null 2>&1 || \
		go get github.com/jteeuwen/go-bindata/...
	glide install

clean:
	rm -rf dist
	rm -f evebox
	rm -f resources/public/*
	rm -f resources/bindata.go
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf vendor
	$(MAKE) -C webapp $@

.PHONY: dist rpm deb

resources/public/app.js resources/public/index.html: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)

resources/public/favicon.ico: resources/favicon.ico
	cp $^ $@
favicon: resources/public/favicon.ico

public: resources/public/index.html resources/public/app.js favicon

resources/bindata.go: RESOURCES := $(shell find resources | grep -v bindata.go)
resources/bindata.go: $(RESOURCES) public
	go generate ./resources/...

# Build's EveBox for the host platform.
evebox: Makefile $(GO_SRCS) resources/bindata.go
	CGO_ENABLED=$(CGO_ENABLED) go build --tags "$(TAGS)" \
		-ldflags "$(LDFLAGS)" -o ${APP} cmd/evebox.go

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
dev-server-reflex:
	reflex -s -R 'bindata\.go' -r '\.go$$' -- \
	sh -c "make evebox && ./evebox --dev http://localhost:58080 ${DEV_ARGS}"

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist: DISTARCH := $(GOARCH)
ifeq ($(GOARCH),amd64)
dist: DISTARCH := x64
endif
ifeq ($(GOARCH),386)
dist: DISTARCH := x32
endif
ifneq ($(VERSION_SUFFIX),)
dist: VERSION := $(VERSION).$(BUILD_DATE_ISO)
endif
dist: DISTNAME ?= ${APP}$(DIST_SUFFIX)-${VERSION}-${GOOS}-${DISTARCH}
dist: LDFLAGS += -s -w
dist: CGO_ENABLED ?= $(CGO_ENABLED)
dist: resources/bindata.go
	CGO_ENABLED=$(CGO_ENABLED) GOARCH=$(GOARCH) GOOS=$(GOOS) \
		go build -tags "$(TAGS)" -ldflags "$(LDFLAGS)" \
		-o dist/$(DISTNAME)/${APP} cmd/evebox.go
	cp agent.toml dist/$(DISTNAME)
	cp evebox-example.yaml dist/$(DISTNAME)
	cd dist && ln -s $(DISTNAME) \
		$(APP)$(DIST_SUFFIX)-latest-$(GOOS)-$(DISTARCH)
	cd dist && zip -r ${DISTNAME}.zip ${DISTNAME}
	cd dist && zip -r $(APP)$(DIST_SUFFIX)-latest-$(GOOS)-$(DISTARCH).zip \
		$(APP)$(DIST_SUFFIX)-latest-$(GOOS)-$(DISTARCH)

release:
	rm -rf dist
	GOOS=linux GOARCH=amd64 $(MAKE) dist
	WITH_SQLITE=1 DIST_SUFFIX="-sqlite" GOOS=linux GOARCH=amd64 $(MAKE) dist
	GOOS=freebsd GOARCH=amd64 $(MAKE) dist
	GOOS=darwin GOARCH=amd64 $(MAKE) dist

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
		dist/${APP}-latest-linux-x64/evebox=/usr/bin/evebox \
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
		--config-files /etc/evebox \
		dist/${APP}-latest-linux-x64/evebox=/usr/bin/evebox \
	        evebox-example.yaml=/etc/evebox/evebox-example.yaml \
		agent.toml=/etc/evebox/agent.toml \
		rpm/evebox.sysconfig=/etc/sysconfig/evebox \
		rpm/evebox.service=/lib/systemd/system/evebox.service \
		rpm/evebox-agent.service=/lib/systemd/system/evebox-agent.service
