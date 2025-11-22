# PPDRIVE CONCEPTS
Some terminologies used in the docs as defined in PPDRIVE context.

## Asset
An asset refers to any file or folder managed by PPDRIVE. This could be uploaded by a user or client.

## User
A user is an individual entity that owns an [Asset](#asset) on PPDRIVE.

## Client
A client typically represents an external application that integrates PPDRIVE by sending requests to manage PPDRIVE's [Assets](#asset) and resources.

## Bucket
A bucket is used to isolate [Assets](#asset) and configure access rules. It also defines the mechanism we use to manage and protect assets management operations(file upload, folder creation...etc).
- Each asset (file or folder) must added to a bucket.
- A bucket may be created by a `Client` or directly by a user.
- A user, if allowed, has a maximum bucket they can create.
- Bucket can be public or private. A public bucket is writable by everyone. A private bucket is writable to only its owner and users with access.
- When writing to bucket, we check if user is bucket owner. If not, we check if it's a public bucket. If not, the operation fails.