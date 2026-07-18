# Java Maven new project

> Create and maintain a conventional Java project with Maven.
> Official documentation: <https://maven.apache.org/guides/getting-started/index.html>.

- Generate the official quickstart project non-interactively:

`mvn archetype:generate -DgroupId={{group_id}} -DartifactId={{project}} -DarchetypeArtifactId=maven-archetype-quickstart -DarchetypeVersion=1.5 -DinteractiveMode=false`

- Compile and run tests:

`cd {{project}} && mvn test`

- Create the distributable package:

`mvn package`

- Show effective configuration when troubleshooting inheritance:

`mvn help:effective-pom`

- Display available dependency updates for review:

`mvn versions:display-dependency-updates`
