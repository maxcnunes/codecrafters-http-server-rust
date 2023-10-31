[![progress-banner](https://backend.codecrafters.io/progress/http-server/0a7ec39c-9adf-471d-ada6-6def4eb3c051)](https://app.codecrafters.io/users/maxcnunes?r=2qF)

My solution in Rust for the
["Build Your Own HTTP server" Challenge](https://app.codecrafters.io/courses/http-server/overview).

[HTTP](https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol) is the
protocol that powers the web. In this challenge, you'll build a HTTP/1.1 server
that is capable of serving multiple clients.

Along the way you'll learn about TCP servers,
[HTTP request syntax](https://www.w3.org/Protocols/rfc2616/rfc2616-sec5.html),
and more.

**Note**: If you're viewing this repo on GitHub, head over to
[codecrafters.io](https://codecrafters.io) to try the challenge.

# Tips

To try this locally on macOS, you could run `./your_server.sh` in one terminal session, and `nc -vz 127.0.0.1 4221` in another. (`-v` gives more verbose output, `-z` just scan for listening daemons, without sending any data to them.)
