## Flow
1. `Server` receives network request from external systems via one of the following (configurable/installable) authetication methods:
- Client Auth
- Direct Auth
- No Auth

2. Server route handler forwards request to approriate `Module`(s) for processing.

3. `Module` processes request, update bookkeeping (for authenticated requests) and send response back to `Server`.

4. `Server` sends response to external systems.

## Servers
We currently working on implementing following servers
1. REST
2. gRPC

## Modules
Modules implement package capabilities:
1. File System Management
    - Upload
    - View
    - Update
    - Delete
2. File Compression
3. File Conversion
4. Image Manipulation

## Packages
- app (bin) - For configuring app, managing and installing app libraries on the server.
- shared: Functionalities shared across the app
