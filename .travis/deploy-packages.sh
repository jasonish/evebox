#! /bin/bash

set -e

API_ROOT="https://api.bintray.com"
REPO_ROOT="https://api.bintray.com/content/jasonish"

if [ "${BINTRAY_API_KEY}" = "" ]; then
    echo "BINTRAY_API_KEY is empty. Not deploying."
    exit 0
fi

deploy_development() {

    # Delete the "latest" version. We do this every time so we don't
    # hit the 180 day limit.
    curl -XDELETE -u jasonish:${BINTRAY_API_KEY} ${API_ROOT}/packages/jasonish/evebox-development/evebox/versions/latest
    printf "\n\n"

    # Upload the "latest" builds.
    for zip in dist/*-latest-*.zip; do
	echo "Uploading ${zip} to version latest."
	curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development/evebox/latest/${dest_filename}"
	printf "\n\n"
    done

    # Deploy zip's to Bintray.
    for zip in dist/evebox-*.zip; do

	version=`echo $(basename ${zip}) | sed -n 's/.*-\([[:digit:]][^-]\+\).*/\1/p'`
	if [ "${version}" = "" ]; then
	    echo "No version found for $zip, skipping."
	    continue
	fi

	echo "Uploading ${zip} with version ${version}."
	curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development/evebox/${version}/$(basename ${zip})"	 
	printf "\n\n"

	# # A bit crude, but also upload with the version of "latest".
	# dest_filename=$(echo $(basename ${zip}) | sed -e "s#${version}#latest#g")

	# echo "Uploading ${zip} to ${dest_filename}."
	# curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	#      -H "X-Bintray-Override: 1" \
	#      -H "X-Bintray-Publish: 1" \
	#      "${REPO_ROOT}/evebox-development/evebox/latest/${dest_filename}"
	# printf "\n\n"

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

deploy_development_debian() {
    for deb in $(ls dist/evebox*_amd64.deb); do
	echo "Uploading ${deb}."
	version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`

	# The old repo - SELKS users may still be downloading from this
	# one.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/deb-evebox-latest/evebox/${version}/$(basename ${deb})"
	echo

	# Debian Jesse / amd64.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
	echo
	
	# Debian Stretch / amd64.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: stretch" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
	echo

	# Debian Stretch / amd64.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: stable,unstable" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
	echo

    done

    for deb in $(ls dist/evebox*_i386.deb); do
	echo "Uploading ${deb}."
	version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: i386" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
    done
    
}

# Only deploy if building from master on my repo.
if [ "${TRAVIS_REPO_SLUG}" != "jasonish/evebox" ]; then
    echo "Not deploying packages for builds from repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

if [ "${TRAVIS_BRANCH}" = "master" ]; then
    deploy_development
    deploy_development_rpm
    deploy_development_debian
fi
