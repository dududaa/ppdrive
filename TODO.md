- ~~Add Client Router tests~~
- ~~Start manager async~~
- ~~Check manager status~~
- ~~Stop manager~~
- ~~Add "Create Client" cli command~~
- ~~implement ppdrive sync~~
- ~~fix/unblock test_create_client~~
- ~~Reinstall dependencies on start~~
- ~~run cargo test~~
- ~~happy clippy~~
- ~~Reduce dylib size~~
- ~~Bundling~~

v0.1.0-beta.2
- ~~Remove source code line number and filename from logging format in stdout~~
- ~~Check manager status by attempting to connect to its TCP stream rather than checking listed services~~
- ~~Plugin installation~~

v0.1.0-beta.3
- ~~Fix logger not working~~
- ~~Fix `ppdrive run rest` failure on Linux.~~
- ~~Fix: `ppdrive start` FileNotFoundError on macOS.~~
- ~~Release notes generated from github actions~~

v0.1.0-rc.1
- ~~fix: validate service start status when a new service is run.~~
- ~~fix: log service start failure.~~
- ~~fix: fix service startup failure.~~
- ~~Add auth modes to service info list~~
- ~~Test Direct router~~
- ~~cli: List tokens~~
- ~~cli: Refresh token~~
- ~~fs: max-bucket-size validation~~
- ~~chore: prevent non-zero max bucket size.~~
- fs: account activation policy
- chore: validate rest API inputs
- chore: use random numbers to create client key instead of uuid
- ~~chore: accept Mb values as f64 from API inputs~~
- cli: validate manager launch


v0.1.0-rc.2
- config: Allow users to load config from filename.
- config: Allow users to save config in filename.
- cli: create admin
- Allow users to configure ppd_secret filename, ppd_log filename, client route base and admin route base.
- Reserve ppd_log, ppd_secret, client_route...etc and prevent assets from using the reserved names.
- rest: admin router 

- Start writing first tutorial
- Implement Direct and Zero auth type for Rest Service
