#! /bin/bash

set -e

REPO_ROOT="https://api.bintray.com/content/jasonish"

# Only deploy if building from master on my repo.
if [ "${TRAVIS_REPO_SLUG}" != "jasonish/evebox" ]; then
    echo "Not deploying packages for builds from repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

if [ "${TRAVIS_BRANCH}" != "master" ]; then
    echo "Not deploying packages for branch ${TRAVIS_BRANCH}."
    exit 0
fi

if [ "${BINTRAY_API_KEY}" = "" ]; then
    echo "BINTRAY_API_KEY is empty. Not deploying."
    exit 0
fi

# Deploy zip's to Bintray.
for zip in dist/evebox-*.zip; do
    version=`echo $(basename ${zip}) | sed -n 's/evebox-\([^-]\+\).*/\1/p'`
    echo "Uploading ${zip}."
    curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	 "${REPO_ROOT}/evebox-development/evebox/${version}/$(basename ${zip})?publish=1&override=1"	 
    echo
done

# OLD - Deploy RPM to Bintray.
for rpm in dist/evebox*.rpm; do
    echo "Uploading ${rpm}."
    version=`rpm -qp --queryformat '%{VERSION}%{RELEASE}' "${rpm}"`

    # Old repo.
    curl -T ${rpm} -u jasonish:${BINTRAY_API_KEY} \
	 "${REPO_ROOT}/evebox-rpm-dev/evebox/${version}/$(basename ${rpm})?publish=1&override=1"

    # New repo.
    curl -T ${rpm} -u jasonish:${BINTRAY_API_KEY} \
	 "${REPO_ROOT}/evebox-development-rpm-x86_64/evebox/${version}/$(basename ${rpm})?publish=1&override=1"

    echo
    break
done

# Deploy Debian package to Bintray.
for deb in dist/evebox*.deb; do
    echo "Uploading ${deb}."
    version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`

    # The old repo - SELKS users may still be downloading from this
    # one.
    curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	 "${REPO_ROOT}/deb-evebox-latest/evebox/${version}/$(basename ${deb});deb_distribution=jessie;deb_component=main;deb_architecture=amd64?publish=1&override=1"
    echo

    # The new repo.
    curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	 -H "X-Bintray-Debian-Distribution: jessie" \
	 -H "X-Bintray-Debian-Component: main" \
	 -H "X-Bintray-Debian-Architecture: amd64" \
	 -H "X-Bintray-Override: 1" \
	 -H "X-Bintray-Publish: 1" \
	 "${REPO_ROOT}/evebox-unstable-debian-x86_64/evebox/${version}/$(basename ${deb})"

    break
done
