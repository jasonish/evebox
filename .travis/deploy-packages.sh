#! /bin/bash

set -e

echo "TRAVIS_BRANCH: ${TRAVIS_BRANCH}"
echo "TRAVIS_TAG: ${TRAVIS_TAG}"
echo "TRAVIS_REPO_SLUG: ${TRAVIS_REPO_SLUG}"

# Only deploy if building from master on my repo.
if [ "${TRAVIS_REPO_SLUG}" != "jasonish/evebox" ]; then
    echo "Not deploying packages for builds from repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

API_ROOT="https://api.bintray.com"
REPO_ROOT="https://api.bintray.com/content/jasonish"

if [ "${BINTRAY_API_KEY}" = "" ]; then
    echo "BINTRAY_API_KEY is empty. Not deploying."
    exit 0
fi

deploy_development() {

    repo="evebox-development"

    # Delete the "latest" version. We do this every time so we don't
    # hit the 180 day limit.
    echo "Deleting latest .tar.gz releases..."
    curl -XDELETE -u jasonish:${BINTRAY_API_KEY} \
	 ${API_ROOT}/packages/jasonish/evebox-development/evebox/versions/latest
    echo

    # Upload the "latest" builds.
    for zip in dist/*-latest-*.zip; do
	echo "Uploading ${zip} to version latest."
	curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/${repo}/evebox/latest/${dest_filename}"
	echo
    done
}

deploy_development_rpm() {
    # Deploy RPM to Bintray.
    for rpm in $(ls dist/evebox*.rpm); do
	echo "Uploading ${rpm}."
	version=`rpm -qp --queryformat '%{VERSION}%{RELEASE}' "${rpm}"`
	curl -T ${rpm} -u jasonish:${BINTRAY_API_KEY} \
	     "${REPO_ROOT}/evebox-development-rpm-x86_64/evebox/${version}/$(basename ${rpm})?publish=1&override=1"
	
	echo
	break
    done
}

deploy_debian() {

    repo="evebox-development-debian"
    distribution="$1"

    if [ "${distribution}" = "" ]; then
	echo "error: deploy-debian: no distribution provided"
	return
    fi

    for deb in $(ls dist/evebox*_amd64.deb); do

	echo "Uploading ${deb} to ${repo}/${distribution}"

	version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`

	# Debian Stretch / amd64.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: ${distribution}" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/${repo}/evebox/${version}/$(basename ${deb})"
	echo

    done
}

if [ "${TRAVIS_BRANCH}" = "master" ]; then
    deploy_development
    deploy_development_rpm
    deploy_debian "unstable,jessie,stretch"
fi

if [ "${TRAVIS_BRANCH}" = "develop" ]; then
    deploy_debian "development"
fi
