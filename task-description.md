## Exercise A: Implement a toy peer-to-peer network

This first exercise asks you to program in a toy example of a peer-to-peer
flooding network for sending strings around. The peer-to-peer network should
then be used to build a distributed chat room. The chat room client should work
as follows: 

1. It runs as a command line program. 
2. When it starts up it asks for the IP address and port number of an
   existing peer on the network. If the IP address or port is invalid or no
   peer is found at the address, the client starts its own new network with
   only itself as member.
3. Then the client prints its own IP address and the port on which it waits
   for connections.
4. Then it will iteratively prompt the user for text strings.
5. When the user types a text string at any connected client, then it will
   eventually be printed at all other clients.
6. Only the text string should be printed, no information about who sent it.

The system should be implemented as follows:

1. When a client connects to an existing peer, it will keep a TCP connection
   to that peer.
2. Then the client opens its own port where it waits for incoming TCP
   connections.
3. All the connections will be treated the same, they will be used for both
   sending and receiving strings.
4. It keeps a set of messages that it already sent.
5. When a string is typed by the user or a string arrives on any of
   its connections, the client checks if it is already sent. If so, it does
   nothing. Otherwise it adds it to MessagesSent and then sends it on all its
   connections. (Remember concurrency control. Probably several routines will
   access the set at the same time. Make sure that does not give problems.)
