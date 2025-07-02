// See https://aka.ms/new-console-template for more information

using System.Text;

StringBuilder sb = new StringBuilder();
sb.AppendLine("It works");
sb.AppendLine($"Current directory: {Environment.CurrentDirectory}");
for (int i = 0; i < args.Length; i++)
{
    sb.AppendLine($"Arg {i}: {args[i]}");
}

if (args.Length == 0)
{
    sb.AppendLine("No args passed");
}


File.WriteAllText(@"D:\\dotnet_works.txt", sb.ToString());

