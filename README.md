# Fintech Platform

[Three-Project Series: Build a Fintech Platform in Rust](https://www.manning.com/liveprojectseries/fintech-platform-ser)

 - [Project 1: Fundamentals and Accounting](https://www.manning.com/liveproject/fundamentals-and-accounting)
 - [Project 2: Core Algorithms and Data Structures](https://www.manning.com/liveproject/core-algorithms-and-data-structures)
 - [Project 3 A Shared Marketplace API](https://www.manning.com/liveproject/shared-marketplace-api)



Future Finance Labs, a fintech scale-up company, wants to expand its customer base by offering
a lightning-fast exchange platform for professional traders.
As its star developer, you’ll use the Rust programming language to create a prototype
of the exchange that will accommodate high-frequency trading
and serve as the foundation for additional future financial products.

You’ll build an interactive command line program that will constitute the core accounting structure
for setting up accounts and manipulating data.

You’ll create a matching engine that enables traders to find the best trading partners
and showcases the blazing-fast core of the exchange platform.

You’ll extend your Rust HTTP API by setting up a warp web service that will interact with
an additional trading platform, by building a shared marketplace that will be a blueprint for
additional Rust web services, small and large.

## Notes
- My implementation is a much improved version of the project's requirements and starter code.
  For example:
  - My implementation contains a much improved version of the matching engine, which is heavily-documented.  
    Additionally, the matching engine contains a vast amount of unit tests.
  - I have generally added more unit tests than required, for various parts of the application.

## Potential Improvements and Additions
- Checking if a seller really has the amount they wish to sell;
- Removal of an account;
- Clearing of everything: all accounts and entire transaction log.
