# Version info.
VERSION		:=	0.6.0
VERSION_SUFFIX	:=	dev
BUILD_DATE	:=	$(shell TZ=UTC date)
BUILD_DATE_ISO	:=	$(shell TZ=UTC date +%Y%m%d%H%M%S)
BUILD_REV	:=	$(shell git rev-parse --short HEAD)

export GO15VENDOREXPERIMENT=1

LDFLAGS :=	-X \"main.buildDate=$(BUILD_DATE)\" \
		-X \"main.buildRev=$(BUILD_REV)\" \
		-X \"main.buildVersion=$(VERSION)$(VERSION_SUFFIX)\" \

APP :=		evebox

WEBAPP_SRCS :=	$(shell find webapp/src -type f)
GO_SRCS :=	$(shell find . -name \*.go)

all: public evebox

install-deps:
# NPM
	$(MAKE) -C webapp $@
# Go
	go get github.com/Masterminds/glide
	go get github.com/GeertJohan/go.rice/rice
	go get github.com/codegangsta/gin
	glide install

clean:
	rm -rf dist
	rm -f evebox
	rm -f public/*.js
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf vendor
	$(MAKE) -C webapp $@

.PHONY: public dist rpm deb

# Build the webapp bundle.
public/bundle.js: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
public: public/bundle.js

# Build's EveBox for the host platform.
evebox: $(GO_SRCS)
	CGO_ENABLED=0 go build -ldflags "$(LDFLAGS)" -o ${APP}

with-docker:
	docker build --rm -t evebox/builder - < Dockerfile
	docker run --rm -it \
		-v `pwd`:/go/src/evebox \
		-w /go/src/evebox \
		evebox/builder make install-deps all

dev-server: evebox
	@if [ "${EVEBOX_ELASTICSEARCH_URL}" = "" ]; then \
		echo "error: EVEBOX_ELASTICSEARCH_URL not set."; \
		exit 1; \
	fi
	./webapp/node_modules/.bin/concurrently -k \
		"make -C webapp start" \
		"gin --appPort 5636 -i -b evebox ./evebox -e ${EVEBOX_ELASTICSEARCH_URL} --dev http://localhost:8080"

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist: DISTNAME ?= ${APP}-${VERSION}${VERSION_SUFFIX}-${GOOS}-${GOARCH}
dist: LDFLAGS += -s -w
dist: public/bundle.js
	GOARCH=$(GOARCH) GOOS=$(GOOS) CGO_ENABLED=0 \
		go build -ldflags "$(LDFLAGS)" -o dist/$(DISTNAME)/${APP}
	rice -v append --exec dist/${DISTNAME}/${APP}
	cd dist && zip -r ${DISTNAME}.zip ${DISTNAME}

release:
	GOOS=linux GOARCH=amd64 $(MAKE) dist
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
		dist/${APP}-${VERSION}${VERSION_SUFFIX}-linux-amd64/evebox=/usr/bin/evebox \
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
		dist/${APP}-${VERSION}${VERSION_SUFFIX}-linux-386/evebox=/usr/bin/evebox \
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
		dist/${APP}-${VERSION}${VERSION_SUFFIX}-linux-amd64/evebox=/usr/bin/evebox \
		rpm/evebox.sysconfig=/etc/sysconfig/evebox \
		rpm/evebox.service=/lib/systemd/system/evebox.service
